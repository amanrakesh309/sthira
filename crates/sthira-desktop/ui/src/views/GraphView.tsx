import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import {
  Share2,
  Search,
  ZoomIn,
  ZoomOut,
  RotateCcw,
  ArrowLeft,
  ExternalLink,
} from 'lucide-react';
import { getKnowledgeGraph } from '../api/ipc';
import type { KnowledgeGraphData, GraphNode, GraphEdge } from '../types/sthira';

interface GraphViewProps {
  onOpenProject: (projectId: string) => void;
  onOpenArtifact: (artifactId: string, projectId?: string) => void;
  onBack: () => void;
}

interface SimNode extends GraphNode {
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  isDragging?: boolean;
}

interface SimEdge extends GraphEdge {
  sourceNode?: SimNode;
  targetNode?: SimNode;
}

export const GraphView: React.FC<GraphViewProps> = ({
  onOpenProject,
  onOpenArtifact,
  onBack,
}) => {
  const [graphData, setGraphData] = useState<KnowledgeGraphData>({ nodes: [], edges: [] });
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Filters
  const [showProjects, setShowProjects] = useState(true);
  const [showPages, setShowPages] = useState(true);
  const [showIdeas, setShowIdeas] = useState(true);
  const [showTasks, setShowTasks] = useState(true);
  const [showSources, setShowSources] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');

  // Selected & Hovered Node
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null);
  const [hoveredNode, setHoveredNode] = useState<SimNode | null>(null);

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const simNodesRef = useRef<SimNode[]>([]);
  const simEdgesRef = useRef<SimEdge[]>([]);
  const animFrameIdRef = useRef<number | null>(null);

  // Viewport transform
  const transformRef = useRef({ x: 0, y: 0, scale: 1 });
  const isPanningRef = useRef(false);
  const panStartRef = useRef({ x: 0, y: 0 });
  const draggedNodeRef = useRef<SimNode | null>(null);

  // Load graph data from backend
  const loadGraph = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await getKnowledgeGraph();
      setGraphData(data);
    } catch (err: any) {
      console.error('Failed to load knowledge graph', err);
      setError(err?.toString() || 'Failed to load knowledge graph');
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadGraph();
  }, [loadGraph]);

  // Determine visibility of a node based on current filter states
  const isNodeVisible = useCallback(
    (node: GraphNode): boolean => {
      if (node.kind === 'project') return showProjects;
      if (node.kind === 'artifact') return showPages;
      if (node.kind === 'object') {
        const ot = (node.object_type || '').toLowerCase();
        if (ot === 'idea') return showIdeas;
        if (ot === 'todo' || ot === 'task') return showTasks;
        if (ot === 'source' || ot === 'decision') return showSources;
        return true;
      }
      return true;
    },
    [showProjects, showPages, showIdeas, showTasks, showSources]
  );

  // Initialize or update simulation nodes & edges when graphData or filters change
  useEffect(() => {
    const canvas = canvasRef.current;
    const width = canvas ? canvas.clientWidth : 800;
    const height = canvas ? canvas.clientHeight : 600;

    const visibleNodesMap = new Map<string, SimNode>();
    const existingNodesMap = new Map<string, SimNode>(
      simNodesRef.current.map((n) => [n.id, n])
    );

    const filteredNodes = graphData.nodes.filter(isNodeVisible);

    filteredNodes.forEach((node, idx) => {
      const existing = existingNodesMap.get(node.id);
      const angle = (idx / Math.max(1, filteredNodes.length)) * 2 * Math.PI;
      const radiusDist = 120 + (idx % 4) * 55;

      let radius = 22;
      if (node.kind === 'project') radius = 34;
      else if (node.kind === 'object') radius = 26;

      const simNode: SimNode = {
        ...node,
        radius,
        x: existing ? existing.x : width / 2 + Math.cos(angle) * radiusDist + (Math.random() - 0.5) * 40,
        y: existing ? existing.y : height / 2 + Math.sin(angle) * radiusDist + (Math.random() - 0.5) * 40,
        vx: existing ? existing.vx : 0,
        vy: existing ? existing.vy : 0,
      };

      visibleNodesMap.set(node.id, simNode);
    });

    const simNodes = Array.from(visibleNodesMap.values());
    simNodesRef.current = simNodes;

    // Build visible edges
    const simEdges: SimEdge[] = [];
    for (const edge of graphData.edges) {
      const sourceNode = visibleNodesMap.get(edge.source);
      const targetNode = visibleNodesMap.get(edge.target);
      if (sourceNode && targetNode) {
        simEdges.push({
          ...edge,
          sourceNode,
          targetNode,
        });
      }
    }
    simEdgesRef.current = simEdges;
  }, [graphData, isNodeVisible]);

  // Simulation step & Canvas render loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let isRunning = true;

    const render = () => {
      if (!isRunning) return;

      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      const rectWidth = rect.width || 800;
      const rectHeight = rect.height || 600;
      const targetWidth = Math.floor(rectWidth * dpr);
      const targetHeight = Math.floor(rectHeight * dpr);

      if (canvas.width !== targetWidth || canvas.height !== targetHeight) {
        canvas.width = targetWidth;
        canvas.height = targetHeight;
      }

      ctx.save();
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, rectWidth, rectHeight);

      const transform = transformRef.current;
      ctx.translate(transform.x, transform.y);
      ctx.scale(transform.scale, transform.scale);

      // --- Force Simulation Step ---
      const nodes = simNodesRef.current;
      const edges = simEdgesRef.current;
      const centerX = rectWidth / 2;
      const centerY = rectHeight / 2;

      // 1. Center gravity & Node repulsion
      for (let i = 0; i < nodes.length; i++) {
        const n1 = nodes[i];
        if (n1.isDragging) continue;

        // Subtle pull toward center
        n1.vx += (centerX - n1.x) * 0.0006;
        n1.vy += (centerY - n1.y) * 0.0006;

        // Repulsion from other nodes
        for (let j = i + 1; j < nodes.length; j++) {
          const n2 = nodes[j];
          const dx = n2.x - n1.x;
          const dy = n2.y - n1.y;
          const distSq = dx * dx + dy * dy + 1;
          const dist = Math.sqrt(distSq);
          const minDist = n1.radius + n2.radius + 30;

          if (dist < 400) {
            const force = (minDist * 40) / distSq;
            const fx = (dx / dist) * force;
            const fy = (dy / dist) * force;

            if (!n1.isDragging) {
              n1.vx -= fx;
              n1.vy -= fy;
            }
            if (!n2.isDragging) {
              n2.vx += fx;
              n2.vy += fy;
            }
          }
        }
      }

      // 2. Spring attraction along edges
      for (const edge of edges) {
        if (!edge.sourceNode || !edge.targetNode) continue;
        const s = edge.sourceNode;
        const t = edge.targetNode;

        const dx = t.x - s.x;
        const dy = t.y - s.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const targetDist = edge.relationship === 'belongs_to' ? 100 : 140;
        const springForce = (dist - targetDist) * 0.008;

        const fx = (dx / dist) * springForce;
        const fy = (dy / dist) * springForce;

        if (!s.isDragging) {
          s.vx += fx;
          s.vy += fy;
        }
        if (!t.isDragging) {
          t.vx -= fx;
          t.vy -= fy;
        }
      }

      // 3. Update positions with damping
      for (const n of nodes) {
        if (n.isDragging) continue;
        n.vx *= 0.88;
        n.vy *= 0.88;
        n.x += n.vx;
        n.y += n.vy;
      }

      // --- Draw Edges ---
      for (const edge of edges) {
        if (!edge.sourceNode || !edge.targetNode) continue;
        const s = edge.sourceNode;
        const t = edge.targetNode;

        const isHighlighted =
          (hoveredNode && (s.id === hoveredNode.id || t.id === hoveredNode.id)) ||
          (selectedNode && (s.id === selectedNode.id || t.id === selectedNode.id));

        ctx.beginPath();
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(t.x, t.y);

        if (edge.relationship === 'belongs_to') {
          ctx.strokeStyle = isHighlighted ? '#1E201C' : '#D5D0C3';
          ctx.lineWidth = isHighlighted ? 2.5 : 1.2;
          ctx.setLineDash([]);
        } else if (edge.relationship === 'supports') {
          ctx.strokeStyle = isHighlighted ? '#059669' : '#10B981';
          ctx.lineWidth = isHighlighted ? 2.5 : 1.8;
          ctx.setLineDash([4, 3]);
        } else if (edge.relationship === 'contradicts') {
          ctx.strokeStyle = isHighlighted ? '#DC2626' : '#EF4444';
          ctx.lineWidth = isHighlighted ? 2.5 : 1.8;
          ctx.setLineDash([4, 3]);
        } else if (edge.relationship === 'extends') {
          ctx.strokeStyle = isHighlighted ? '#4F46E5' : '#6366F1';
          ctx.lineWidth = isHighlighted ? 2.5 : 1.8;
          ctx.setLineDash([5, 3]);
        } else {
          // references or default
          ctx.strokeStyle = isHighlighted ? '#D97706' : '#F59E0B';
          ctx.lineWidth = isHighlighted ? 2 : 1.4;
          ctx.setLineDash([2, 3]);
        }
        ctx.stroke();
        ctx.setLineDash([]);

        // Relationship label on non-membership edges
        if (edge.relationship !== 'belongs_to' && (isHighlighted || transform.scale > 0.85)) {
          const midX = (s.x + t.x) / 2;
          const midY = (s.y + t.y) / 2;
          ctx.save();
          ctx.font = '9px system-ui, sans-serif';
          ctx.fillStyle = '#6E6D66';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          const textWidth = ctx.measureText(edge.relationship).width;
          ctx.fillStyle = '#FAF9F5';
          ctx.fillRect(midX - textWidth / 2 - 3, midY - 6, textWidth + 6, 12);
          ctx.strokeStyle = '#E8E5DC';
          ctx.lineWidth = 1;
          ctx.strokeRect(midX - textWidth / 2 - 3, midY - 6, textWidth + 6, 12);
          ctx.fillStyle = '#3A3B36';
          ctx.fillText(edge.relationship, midX, midY);
          ctx.restore();
        }
      }

      // --- Draw Nodes ---
      const query = searchQuery.trim().toLowerCase();

      for (const node of nodes) {
        const isHovered = hoveredNode?.id === node.id;
        const isSelected = selectedNode?.id === node.id;
        const isMatch = query ? node.label.toLowerCase().includes(query) : true;

        ctx.save();

        // Node Glow / Selection Ring
        if (isSelected || (query && isMatch)) {
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.radius + 6, 0, 2 * Math.PI);
          ctx.fillStyle = isSelected ? 'rgba(40, 44, 37, 0.12)' : 'rgba(99, 102, 241, 0.15)';
          ctx.fill();
          ctx.strokeStyle = isSelected ? '#282C25' : '#6366F1';
          ctx.lineWidth = 1.5;
          ctx.stroke();
        }

        // Base Circle
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius, 0, 2 * Math.PI);

        if (node.kind === 'project') {
          // Large Hub Node
          ctx.fillStyle = '#282C25';
          ctx.fill();
          ctx.lineWidth = isHovered ? 3 : 2;
          ctx.strokeStyle = isHovered ? '#6E6D66' : '#3A3B36';
          ctx.stroke();
        } else if (node.kind === 'object') {
          // Typed Object Node
          ctx.fillStyle = '#FAF9F5';
          ctx.fill();
          ctx.lineWidth = isHovered ? 2.5 : 1.8;
          ctx.strokeStyle = node.color || '#6366F1';
          ctx.stroke();
        } else {
          // Standard Page / Artifact Note
          ctx.fillStyle = '#FFFFFF';
          ctx.fill();
          ctx.lineWidth = isHovered ? 2.2 : 1.5;
          ctx.strokeStyle = isHovered ? '#1E201C' : '#E8E5DC';
          ctx.stroke();
        }

        // Node Icon
        ctx.font = `${node.kind === 'project' ? 18 : 14}px system-ui, Apple Color Emoji, Segoe UI Emoji`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(node.icon || (node.kind === 'project' ? '📂' : '📄'), node.x, node.y);

        // Draft indicator on notes
        if (node.draft_status) {
          ctx.beginPath();
          ctx.arc(node.x + node.radius - 4, node.y - node.radius + 4, 4, 0, 2 * Math.PI);
          ctx.fillStyle = '#F59E0B';
          ctx.fill();
          ctx.strokeStyle = '#FFFFFF';
          ctx.lineWidth = 1.5;
          ctx.stroke();
        }

        // Node Label
        ctx.font = `${node.kind === 'project' ? 'bold 12px' : '11px'} system-ui, -apple-system, sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'top';

        const label = node.label.length > 28 ? node.label.slice(0, 26) + '…' : node.label;

        // Label pill background for crisp readability
        const labelWidth = ctx.measureText(label).width;
        const labelY = node.y + node.radius + 6;

        ctx.fillStyle = 'rgba(245, 243, 236, 0.85)';
        ctx.fillRect(node.x - labelWidth / 2 - 4, labelY - 2, labelWidth + 8, 16);

        ctx.fillStyle = node.kind === 'project' ? '#1E201C' : '#3A3B36';
        ctx.fillText(label, node.x, labelY);

        // Item count pill for projects
        if (node.kind === 'project' && node.item_count !== undefined) {
          const subText = `${node.item_count} ${node.item_count === 1 ? 'item' : 'items'}`;
          ctx.font = '10px system-ui, sans-serif';
          ctx.fillStyle = '#8C8A82';
          ctx.fillText(subText, node.x, labelY + 16);
        }

        ctx.restore();
      }

      ctx.restore();

      animFrameIdRef.current = requestAnimationFrame(render);
    };

    render();

    return () => {
      isRunning = false;
      if (animFrameIdRef.current) {
        cancelAnimationFrame(animFrameIdRef.current);
      }
    };
  }, [searchQuery, hoveredNode, selectedNode]);

  // Coordinate conversion helper from client to canvas-sim space
  const getCanvasCoords = useCallback((clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const rect = canvas.getBoundingClientRect();
    const transform = transformRef.current;
    const rawX = clientX - rect.left;
    const rawY = clientY - rect.top;
    return {
      x: (rawX - transform.x) / transform.scale,
      y: (rawY - transform.y) / transform.scale,
    };
  }, []);

  const getNodeAtCoords = useCallback(
    (canvasX: number, canvasY: number): SimNode | null => {
      const nodes = simNodesRef.current;
      for (let i = nodes.length - 1; i >= 0; i--) {
        const node = nodes[i];
        const dx = node.x - canvasX;
        const dy = node.y - canvasY;
        if (dx * dx + dy * dy <= (node.radius + 6) * (node.radius + 6)) {
          return node;
        }
      }
      return null;
    },
    []
  );

  // Mouse interaction handlers
  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const coords = getCanvasCoords(e.clientX, e.clientY);
    const hitNode = getNodeAtCoords(coords.x, coords.y);

    if (hitNode) {
      draggedNodeRef.current = hitNode;
      hitNode.isDragging = true;
      setSelectedNode(hitNode);
    } else {
      isPanningRef.current = true;
      panStartRef.current = {
        x: e.clientX - transformRef.current.x,
        y: e.clientY - transformRef.current.y,
      };
    }
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const coords = getCanvasCoords(e.clientX, e.clientY);

    if (draggedNodeRef.current) {
      draggedNodeRef.current.x = coords.x;
      draggedNodeRef.current.y = coords.y;
      draggedNodeRef.current.vx = 0;
      draggedNodeRef.current.vy = 0;
    } else if (isPanningRef.current) {
      transformRef.current.x = e.clientX - panStartRef.current.x;
      transformRef.current.y = e.clientY - panStartRef.current.y;
    } else {
      const hitNode = getNodeAtCoords(coords.x, coords.y);
      setHoveredNode(hitNode);
    }
  };

  const handleMouseUp = () => {
    if (draggedNodeRef.current) {
      draggedNodeRef.current.isDragging = false;
      draggedNodeRef.current = null;
    }
    isPanningRef.current = false;
  };

  const handleWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const zoomFactor = e.deltaY < 0 ? 1.1 : 0.9;
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const currentScale = transformRef.current.scale;
    const nextScale = Math.min(2.5, Math.max(0.3, currentScale * zoomFactor));

    transformRef.current.x = mouseX - (mouseX - transformRef.current.x) * (nextScale / currentScale);
    transformRef.current.y = mouseY - (mouseY - transformRef.current.y) * (nextScale / currentScale);
    transformRef.current.scale = nextScale;
  };

  const handleDoubleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const coords = getCanvasCoords(e.clientX, e.clientY);
    const hitNode = getNodeAtCoords(coords.x, coords.y);
    if (hitNode) {
      handleNavigateToNode(hitNode);
    }
  };

  const handleNavigateToNode = (node: SimNode) => {
    if (node.kind === 'project') {
      onOpenProject(node.id);
    } else {
      onOpenArtifact(node.id, node.project_id || undefined);
    }
  };

  const resetZoom = () => {
    transformRef.current = { x: 0, y: 0, scale: 1 };
  };

  const zoomIn = () => {
    transformRef.current.scale = Math.min(2.5, transformRef.current.scale * 1.25);
  };

  const zoomOut = () => {
    transformRef.current.scale = Math.max(0.3, transformRef.current.scale * 0.8);
  };

  const visibleNodes = useMemo(
    () => graphData.nodes.filter(isNodeVisible),
    [graphData.nodes, isNodeVisible]
  );

  const visibleEdges = useMemo(() => {
    const visibleIds = new Set(visibleNodes.map((n) => n.id));
    return graphData.edges.filter(
      (e) => visibleIds.has(e.source) && visibleIds.has(e.target)
    );
  }, [graphData.edges, visibleNodes]);

  const totalVisibleNodes = visibleNodes.length;
  const totalVisibleEdges = visibleEdges.length;

  return (
    <div
      data-testid="graph-view-container"
      className="flex flex-col h-screen w-full bg-[#F5F3EC] text-[#1E201C] overflow-hidden select-none"
    >
      {/* Top Header & Interactive Filter Bar */}
      <header className="bg-[#FFFFFF] border-b border-[#E8E5DC] px-6 py-3 shrink-0 flex flex-wrap items-center justify-between gap-3 shadow-2xs z-20">
        {/* Left: Breadcrumbs & Stats */}
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onBack}
            className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-[#EFECE4] hover:bg-[#E5E1D6] text-xs font-semibold text-[#1E201C] transition-colors cursor-pointer"
            title="Return to Home"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Home</span>
          </button>

          <span className="text-[#8C8A82] text-xs font-medium">/</span>

          <div className="flex items-center gap-2">
            <Share2 className="w-4 h-4 text-[#282C25]" />
            <h1 className="text-sm font-bold text-[#1E201C] tracking-tight">
              Knowledge Graph Map
            </h1>
          </div>

          <div className="hidden md:flex items-center gap-1.5 text-[11px] text-[#6E6D66] bg-[#FAF9F5] border border-[#E8E5DC] px-2.5 py-1 rounded-lg">
            <span data-testid="graph-node-count" className="font-semibold text-[#1E201C]">
              {totalVisibleNodes}
            </span>{' '}
            nodes
            <span className="text-[#8C8A82]">·</span>
            <span data-testid="graph-edge-count" className="font-semibold text-[#1E201C]">
              {totalVisibleEdges}
            </span>{' '}
            edges
          </div>
        </div>

        {/* Center: Filter Pills */}
        <div className="flex items-center gap-1.5 flex-wrap">
          <span className="text-[11px] font-bold text-[#8C8A82] uppercase tracking-wider mr-1 hidden lg:inline">
            Filters:
          </span>

          <button
            type="button"
            data-testid="filter-projects"
            onClick={() => setShowProjects((prev) => !prev)}
            className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer border flex items-center gap-1.5 ${
              showProjects
                ? 'bg-[#282C25] text-white border-[#282C25]'
                : 'bg-white text-[#6E6D66] border-[#E8E5DC] hover:bg-[#FAF9F5]'
            }`}
          >
            <span>📂</span>
            <span>Projects</span>
          </button>

          <button
            type="button"
            data-testid="filter-pages"
            onClick={() => setShowPages((prev) => !prev)}
            className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer border flex items-center gap-1.5 ${
              showPages
                ? 'bg-[#282C25] text-white border-[#282C25]'
                : 'bg-white text-[#6E6D66] border-[#E8E5DC] hover:bg-[#FAF9F5]'
            }`}
          >
            <span>📄</span>
            <span>Pages</span>
          </button>

          <button
            type="button"
            data-testid="filter-ideas"
            onClick={() => setShowIdeas((prev) => !prev)}
            className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer border flex items-center gap-1.5 ${
              showIdeas
                ? 'bg-[#282C25] text-white border-[#282C25]'
                : 'bg-white text-[#6E6D66] border-[#E8E5DC] hover:bg-[#FAF9F5]'
            }`}
          >
            <span>💡</span>
            <span>Ideas</span>
          </button>

          <button
            type="button"
            data-testid="filter-tasks"
            onClick={() => setShowTasks((prev) => !prev)}
            className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer border flex items-center gap-1.5 ${
              showTasks
                ? 'bg-[#282C25] text-white border-[#282C25]'
                : 'bg-white text-[#6E6D66] border-[#E8E5DC] hover:bg-[#FAF9F5]'
            }`}
          >
            <span>☑️</span>
            <span>Tasks</span>
          </button>

          <button
            type="button"
            data-testid="filter-sources"
            onClick={() => setShowSources((prev) => !prev)}
            className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer border flex items-center gap-1.5 ${
              showSources
                ? 'bg-[#282C25] text-white border-[#282C25]'
                : 'bg-white text-[#6E6D66] border-[#E8E5DC] hover:bg-[#FAF9F5]'
            }`}
          >
            <span>🔗</span>
            <span>Sources</span>
          </button>
        </div>

        {/* Right: Search & Actions */}
        <div className="flex items-center gap-2">
          <div className="relative">
            <Search className="w-3.5 h-3.5 text-[#8C8A82] absolute left-2.5 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              data-testid="graph-search-input"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Filter nodes..."
              className="pl-8 pr-2.5 py-1 text-xs rounded-xl border border-[#E8E5DC] bg-[#FAF9F5] outline-none focus:border-[#282C25] w-32 sm:w-44"
            />
          </div>

          <div className="flex items-center border border-[#E8E5DC] rounded-xl overflow-hidden bg-white">
            <button
              type="button"
              onClick={zoomIn}
              title="Zoom In"
              className="p-1.5 text-[#6E6D66] hover:bg-[#EFECE4] hover:text-[#1E201C] transition-colors cursor-pointer"
            >
              <ZoomIn className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={zoomOut}
              title="Zoom Out"
              className="p-1.5 text-[#6E6D66] hover:bg-[#EFECE4] hover:text-[#1E201C] transition-colors cursor-pointer border-l border-[#E8E5DC]"
            >
              <ZoomOut className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={resetZoom}
              title="Reset View"
              className="p-1.5 text-[#6E6D66] hover:bg-[#EFECE4] hover:text-[#1E201C] transition-colors cursor-pointer border-l border-[#E8E5DC]"
            >
              <RotateCcw className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      </header>

      {/* Main Interactive Canvas Area */}
      <div className="flex-1 relative w-full h-full overflow-hidden bg-[#F5F3EC]">
        {isLoading ? (
          <div className="absolute inset-0 flex items-center justify-center bg-[#F5F3EC]/80 z-10 text-xs text-[#8C8A82]">
            Loading knowledge graph map...
          </div>
        ) : error ? (
          <div className="absolute inset-0 flex items-center justify-center bg-[#F5F3EC] z-10 p-6">
            <div className="p-4 rounded-xl bg-red-50 border border-red-200 text-red-700 text-xs max-w-md">
              {error}
            </div>
          </div>
        ) : totalVisibleNodes === 0 ? (
          <div className="absolute inset-0 flex flex-col items-center justify-center text-center p-6 text-[#8C8A82]">
            <Share2 className="w-10 h-10 mb-2 stroke-1" />
            <h3 className="text-sm font-semibold text-[#1E201C]">No matching graph nodes</h3>
            <p className="text-xs max-w-sm mt-1">
              Adjust your filters or create projects and pages to visualize knowledge relationships.
            </p>
          </div>
        ) : null}

        <canvas
          ref={canvasRef}
          data-testid="graph-canvas"
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onWheel={handleWheel}
          onDoubleClick={handleDoubleClick}
          className="w-full h-full cursor-grab active:cursor-grabbing block"
        />

        {/* Floating Selected Node Details Card */}
        {selectedNode && (
          <div className="absolute bottom-6 right-6 z-20 w-80 bg-white/95 backdrop-blur-sm border border-[#E8E5DC] rounded-2xl shadow-xl p-4 text-xs animate-in slide-in-from-bottom-2 duration-150">
            <div className="flex items-start justify-between gap-2 mb-2">
              <div className="flex items-center gap-2">
                <span className="text-lg">{selectedNode.icon || '📄'}</span>
                <div>
                  <h4 className="font-bold text-[#1E201C] line-clamp-1">{selectedNode.label}</h4>
                  <span className="text-[10px] uppercase font-semibold text-[#8C8A82] tracking-wider">
                    {selectedNode.kind === 'object'
                      ? `${selectedNode.object_type || 'object'}`
                      : selectedNode.kind}
                  </span>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setSelectedNode(null)}
                className="text-[#8C8A82] hover:text-[#1E201C] p-0.5 rounded cursor-pointer"
              >
                ✕
              </button>
            </div>

            {selectedNode.kind === 'project' && selectedNode.item_count !== undefined && (
              <p className="text-[#6E6D66] text-[11px] mb-3">
                This project contains <strong>{selectedNode.item_count}</strong> attached pages and entities.
              </p>
            )}

            {selectedNode.kind !== 'project' && (
              <p className="text-[#6E6D66] text-[11px] mb-3">
                {selectedNode.draft_status
                  ? 'Unfinished draft note ready for continuation.'
                  : 'Synthesized knowledge artifact in workspace.'}
              </p>
            )}

            <button
              type="button"
              data-testid="btn-open-selected-node"
              onClick={() => handleNavigateToNode(selectedNode)}
              className="w-full py-1.5 px-3 rounded-xl bg-[#282C25] hover:opacity-90 text-white font-semibold text-xs flex items-center justify-center gap-1.5 transition-opacity cursor-pointer shadow-xs"
            >
              <span>{selectedNode.kind === 'project' ? 'Open Project' : 'Focus Note'}</span>
              <ExternalLink className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {/* Legend Overlay at Bottom-Left */}
        <div className="absolute bottom-6 left-6 z-10 bg-white/90 backdrop-blur-xs border border-[#E8E5DC] rounded-xl px-3 py-2 text-[11px] text-[#6E6D66] shadow-xs hidden sm:flex items-center gap-4">
          <div className="flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full bg-[#282C25]" />
            <span>Project</span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full bg-white border border-[#E8E5DC]" />
            <span>Note</span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full bg-[#6366F1]" />
            <span>Object</span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="w-3 h-0.5 bg-[#10B981]" />
            <span>Semantic Link</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default GraphView;
