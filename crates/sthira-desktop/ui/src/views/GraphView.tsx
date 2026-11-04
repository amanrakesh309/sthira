import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import {
  Share2,
  Search,
  SlidersHorizontal,
  Maximize2,
  MoreHorizontal,
  ExternalLink,
  Info,
  Check,
  RotateCcw,
} from 'lucide-react';
import { getKnowledgeGraph, listObjectTypes } from '../api/ipc';
import type { KnowledgeGraphData, GraphNode, GraphEdge, ObjectType } from '../types/sthira';

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
  cardWidth?: number;
  cardHeight?: number;
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
  const [objectTypes, setObjectTypes] = useState<ObjectType[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Filters
  const [showProjects, setShowProjects] = useState(true);
  const [showPages, setShowPages] = useState(true);
  const [showIdeas, setShowIdeas] = useState(true);
  const [showTasks, setShowTasks] = useState(true);
  const [showSources, setShowSources] = useState(true);
  const [selectedCustomTypes, setSelectedCustomTypes] = useState<Record<string, boolean>>({});
  const [searchQuery, setSearchQuery] = useState('');

  // Selected & Hovered Node
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null);
  const [hoveredNode, setHoveredNode] = useState<SimNode | null>(null);
  const [zoomScale, setZoomScale] = useState(1);

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const minimapCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  const simNodesRef = useRef<SimNode[]>([]);
  const simEdgesRef = useRef<SimEdge[]>([]);
  const animFrameIdRef = useRef<number | null>(null);

  // Viewport transform
  const transformRef = useRef({ x: 0, y: 0, scale: 1 });
  const isPanningRef = useRef(false);
  const panStartRef = useRef({ x: 0, y: 0 });
  const draggedNodeRef = useRef<SimNode | null>(null);

  // Load graph data & registered types from backend
  const loadGraph = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const [data, types] = await Promise.all([
        getKnowledgeGraph(),
        listObjectTypes().catch(() => [] as ObjectType[]),
      ]);
      setGraphData(data || { nodes: [], edges: [] });
      setObjectTypes(Array.isArray(types) ? types : []);
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

  // Global keyboard shortcuts (⌘F / Ctrl+F for search, Escape to clear)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'f') {
        const activeTag = document.activeElement?.tagName;
        if (activeTag !== 'INPUT' && activeTag !== 'TEXTAREA') {
          e.preventDefault();
          searchInputRef.current?.focus();
        }
      } else if (e.key === 'Escape') {
        if (selectedNode) {
          setSelectedNode(null);
        } else if (searchQuery) {
          setSearchQuery('');
        } else if (onBack) {
          onBack();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedNode, searchQuery]);

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
        if (selectedCustomTypes[ot] !== undefined) {
          return selectedCustomTypes[ot];
        }
        return true;
      }
      return true;
    },
    [showProjects, showPages, showIdeas, showTasks, showSources, selectedCustomTypes]
  );

  // Initialize or update simulation nodes & edges when graphData or filters change
  useEffect(() => {
    const canvas = canvasRef.current;
    const width = canvas ? canvas.clientWidth : 900;
    const height = canvas ? canvas.clientHeight : 650;

    const visibleNodesMap = new Map<string, SimNode>();
    const existingNodesMap = new Map<string, SimNode>(
      simNodesRef.current.map((n) => [n.id, n])
    );

    const filteredNodes = graphData.nodes.filter(isNodeVisible);

    // Group items around their projects if possible
    const projectPositions = new Map<string, { x: number; y: number }>();
    const projectNodes = filteredNodes.filter((n) => n.kind === 'project');

    projectNodes.forEach((pNode, pIdx) => {
      const existing = existingNodesMap.get(pNode.id);
      if (existing) {
        projectPositions.set(pNode.id, { x: existing.x, y: existing.y });
      } else {
        const angle = (pIdx / Math.max(1, projectNodes.length)) * 2 * Math.PI - Math.PI / 4;
        const dist = 180 + (pIdx % 3) * 60;
        projectPositions.set(pNode.id, {
          x: width / 2 + Math.cos(angle) * dist,
          y: height / 2 + Math.sin(angle) * dist,
        });
      }
    });

    filteredNodes.forEach((node, idx) => {
      const existing = existingNodesMap.get(node.id);

      let radius = 24;
      let cardWidth = 180;
      const cardHeight = 42;

      if (node.kind === 'project') {
        radius = 34;
      } else {
        // Approximate card width based on label length
        const labelLen = node.label.length;
        cardWidth = Math.min(240, Math.max(150, labelLen * 7 + 60));
        radius = cardWidth / 2;
      }

      let initX = width / 2;
      let initY = height / 2;

      if (existing) {
        initX = existing.x;
        initY = existing.y;
      } else if (node.kind === 'project') {
        const pos = projectPositions.get(node.id);
        if (pos) {
          initX = pos.x;
          initY = pos.y;
        }
      } else if (node.project_id && projectPositions.has(node.project_id)) {
        const parentPos = projectPositions.get(node.project_id)!;
        const angle = (idx * 1.37) % (2 * Math.PI);
        const dist = 130 + (idx % 3) * 40;
        initX = parentPos.x + Math.cos(angle) * dist + (Math.random() - 0.5) * 30;
        initY = parentPos.y + Math.sin(angle) * dist + (Math.random() - 0.5) * 30;
      } else {
        const angle = (idx / Math.max(1, filteredNodes.length)) * 2 * Math.PI;
        const dist = 220 + (idx % 4) * 50;
        initX = width / 2 + Math.cos(angle) * dist + (Math.random() - 0.5) * 40;
        initY = height / 2 + Math.sin(angle) * dist + (Math.random() - 0.5) * 40;
      }

      const simNode: SimNode = {
        ...node,
        radius,
        cardWidth,
        cardHeight,
        x: initX,
        y: initY,
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

  // Canvas render loop & Physics step
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let isRunning = true;

    // Helper for rounded rect on canvas with jsdom test environment compatibility
    const drawRoundRect = (
      c: CanvasRenderingContext2D,
      x: number,
      y: number,
      w: number,
      h: number,
      r: number
    ) => {
      if (typeof c.roundRect === 'function') {
        c.beginPath();
        c.roundRect(x, y, w, h, r);
        return;
      }
      if (typeof c.arcTo === 'function') {
        const rad = Math.min(r, w / 2, h / 2);
        c.beginPath();
        c.moveTo(x + rad, y);
        c.arcTo(x + w, y, x + w, y + h, rad);
        c.arcTo(x + w, y + h, x, y + h, rad);
        c.arcTo(x, y + h, x, y, rad);
        c.arcTo(x, y, x + w, y, rad);
        c.closePath();
        return;
      }
      if (typeof c.rect === 'function') {
        c.beginPath();
        c.rect(x, y, w, h);
        c.closePath();
        return;
      }
      if (typeof c.beginPath === 'function') {
        c.beginPath();
        if (typeof c.closePath === 'function') c.closePath();
      }
    };

    const render = () => {
      if (!isRunning) return;

      const isDark = document.documentElement.classList.contains('dark');
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      const rectWidth = rect.width || 900;
      const rectHeight = rect.height || 650;
      const targetWidth = Math.floor(rectWidth * dpr);
      const targetHeight = Math.floor(rectHeight * dpr);

      if (canvas.width !== targetWidth || canvas.height !== targetHeight) {
        canvas.width = targetWidth;
        canvas.height = targetHeight;
      }

      ctx.save();
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, rectWidth, rectHeight);

      // Draw subtle warm canvas background
      ctx.fillStyle = isDark ? '#161714' : '#FAF8F3';
      ctx.fillRect(0, 0, rectWidth, rectHeight);

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
        const gravityWeight = n1.kind === 'project' ? 0.0007 : 0.0004;
        n1.vx += (centerX - n1.x) * gravityWeight;
        n1.vy += (centerY - n1.y) * gravityWeight;

        // Repulsion from other nodes
        for (let j = i + 1; j < nodes.length; j++) {
          const n2 = nodes[j];
          const dx = n2.x - n1.x;
          const dy = n2.y - n1.y;
          const distSq = dx * dx + dy * dy + 1;
          const dist = Math.sqrt(distSq);

          const r1 = n1.kind === 'project' ? 50 : (n1.cardWidth || 160) / 2;
          const r2 = n2.kind === 'project' ? 50 : (n2.cardWidth || 160) / 2;
          const minDist = r1 + r2 + (n1.kind === 'project' || n2.kind === 'project' ? 60 : 35);

          if (dist < 450) {
            const force = (minDist * 45) / distSq;
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
        const targetDist = edge.relationship === 'belongs_to' ? 140 : 180;
        const springForce = (dist - targetDist) * 0.007;

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
        n.vx *= 0.86;
        n.vy *= 0.86;
        n.x += n.vx;
        n.y += n.vy;
      }

      // --- Draw Edges (Organic Bezier Splines) ---
      for (const edge of edges) {
        if (!edge.sourceNode || !edge.targetNode) continue;
        const s = edge.sourceNode;
        const t = edge.targetNode;

        const isHighlighted =
          (hoveredNode && (s.id === hoveredNode.id || t.id === hoveredNode.id)) ||
          (selectedNode && (s.id === selectedNode.id || t.id === selectedNode.id));

        ctx.save();
        ctx.beginPath();
        ctx.moveTo(s.x, s.y);

        // Smooth organic curved bezier connection
        const dx = t.x - s.x;
        const dy = t.y - s.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const curvature = Math.min(30, dist * 0.14);
        const sign = ((s.x * 3 + t.y * 7) | 0) % 2 === 0 ? 1 : -1;
        const perpX = (-dy / dist) * curvature * sign;
        const perpY = (dx / dist) * curvature * sign;
        const cpX = (s.x + t.x) / 2 + perpX;
        const cpY = (s.y + t.y) / 2 + perpY;

        if (typeof ctx.quadraticCurveTo === 'function') {
          ctx.quadraticCurveTo(cpX, cpY, t.x, t.y);
        } else {
          ctx.lineTo(t.x, t.y);
        }

        const setDash = (dash: number[]) => {
          if (typeof ctx.setLineDash === 'function') {
            ctx.setLineDash(dash);
          }
        };

        if (edge.relationship === 'belongs_to') {
          ctx.strokeStyle = isHighlighted
            ? (isDark ? '#879E78' : '#384232')
            : (isDark ? 'rgba(100, 105, 95, 0.45)' : 'rgba(200, 194, 179, 0.7)');
          ctx.lineWidth = isHighlighted ? 2.5 : 1.4;
          setDash([]);
        } else if (edge.relationship === 'supports') {
          ctx.strokeStyle = isHighlighted ? '#10B981' : (isDark ? '#059669' : '#34D399');
          ctx.lineWidth = isHighlighted ? 2.5 : 1.6;
          setDash([4, 3]);
        } else if (edge.relationship === 'contradicts') {
          ctx.strokeStyle = isHighlighted ? '#EF4444' : '#F87171';
          ctx.lineWidth = isHighlighted ? 2.5 : 1.6;
          setDash([4, 3]);
        } else if (edge.relationship === 'extends') {
          ctx.strokeStyle = isHighlighted ? '#6366F1' : '#818CF8';
          ctx.lineWidth = isHighlighted ? 2.5 : 1.6;
          setDash([5, 3]);
        } else {
          ctx.strokeStyle = isHighlighted ? '#F59E0B' : (isDark ? '#B45309' : '#FBBF24');
          ctx.lineWidth = isHighlighted ? 2.2 : 1.4;
          setDash([3, 3]);
        }
        ctx.stroke();
        setDash([]);

        // Relationship label on non-membership edges
        if (edge.relationship !== 'belongs_to' && (isHighlighted || transform.scale > 0.9)) {
          const edgeMidX = (s.x + t.x) / 2;
          const edgeMidY = (s.y + t.y) / 2;
          ctx.font = '10px Inter, system-ui, sans-serif';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          const textWidth = ctx.measureText(edge.relationship).width;

          ctx.fillStyle = isDark ? '#1C1D19' : '#FAF8F3';
          drawRoundRect(ctx, edgeMidX - textWidth / 2 - 5, edgeMidY - 8, textWidth + 10, 16, 4);
          ctx.fill();
          ctx.strokeStyle = isDark ? '#2C2E27' : '#E8E5DC';
          ctx.lineWidth = 1;
          ctx.stroke();

          ctx.fillStyle = isDark ? '#EDEAE2' : '#3A3B36';
          ctx.fillText(edge.relationship, edgeMidX, edgeMidY);
        }

        ctx.restore();
      }

      // --- Draw Nodes ---
      const query = searchQuery.trim().toLowerCase();

      for (const node of nodes) {
        const isHovered = hoveredNode?.id === node.id;
        const isSelected = selectedNode?.id === node.id;
        const isMatch = query ? node.label.toLowerCase().includes(query) : true;

        ctx.save();

        if (query && !isMatch) {
          ctx.globalAlpha = 0.25;
        }

        // ==========================================
        // 1. PROJECT HUB NODE RENDERING
        // ==========================================
        if (node.kind === 'project') {
          // Outer soft concentric glowing halo
          const outerHaloRadius = node.radius * 3.1;
          if (typeof ctx.createRadialGradient === 'function') {
            const grad = ctx.createRadialGradient(
              node.x,
              node.y,
              node.radius * 0.7,
              node.x,
              node.y,
              outerHaloRadius
            );
            grad.addColorStop(0, isDark ? 'rgba(75, 95, 65, 0.4)' : 'rgba(215, 207, 190, 0.55)');
            grad.addColorStop(0.5, isDark ? 'rgba(50, 65, 45, 0.2)' : 'rgba(230, 224, 210, 0.35)');
            grad.addColorStop(1, isDark ? 'rgba(22, 23, 20, 0)' : 'rgba(250, 248, 243, 0)');
            ctx.beginPath();
            ctx.arc(node.x, node.y, outerHaloRadius, 0, 2 * Math.PI);
            ctx.fillStyle = grad;
            ctx.fill();
          }

          // Middle concentric ring
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.radius * 1.75, 0, 2 * Math.PI);
          ctx.fillStyle = isDark ? 'rgba(75, 95, 65, 0.25)' : 'rgba(220, 212, 196, 0.45)';
          ctx.fill();

          // Selection / Hover highlight ring
          if (isSelected || isHovered) {
            ctx.beginPath();
            ctx.arc(node.x, node.y, node.radius + 6, 0, 2 * Math.PI);
            ctx.strokeStyle = isSelected
              ? (isDark ? '#879E78' : '#282C25')
              : (isDark ? '#6E7F60' : '#49583D');
            ctx.lineWidth = 2;
            ctx.stroke();
          }

          // Core Hub Circle (Forest Sage)
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.radius, 0, 2 * Math.PI);
          ctx.fillStyle = isDark ? '#2E3829' : '#384232';
          ctx.fill();
          ctx.strokeStyle = isDark ? '#4A5B40' : '#2A3325';
          ctx.lineWidth = 2;
          ctx.stroke();

          // Gold folder icon inside hub
          ctx.font = '20px Apple Color Emoji, Segoe UI Emoji, system-ui';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          ctx.fillText(node.icon || '📁', node.x, node.y);

          // Project Title below hub
          ctx.font = 'bold 13px Fraunces, "Source Serif 4", Georgia, serif';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'top';
          ctx.fillStyle = isDark ? '#EDEAE2' : '#1E201C';

          const label = node.label.length > 28 ? node.label.slice(0, 26) + '…' : node.label;
          ctx.fillText(label, node.x, node.y + node.radius + 10);

          // Subtitle / item count
          if (node.item_count !== undefined) {
            ctx.font = '11px Inter, system-ui, sans-serif';
            ctx.fillStyle = isDark ? '#9E9C93' : '#73766D';
            ctx.fillText(
              `${node.item_count} ${node.item_count === 1 ? 'item' : 'items'}`,
              node.x,
              node.y + node.radius + 28
            );
          }
        } else {
          // ==========================================
          // 2. NOTE / PAGE / OBJECT CARD PILL RENDERING
          // ==========================================
          const cardW = node.cardWidth || 170;
          const cardH = node.cardHeight || 42;
          const cardX = node.x - cardW / 2;
          const cardY = node.y - cardH / 2;
          const cardR = 12;

          // Card Shadow
          ctx.shadowColor = isDark ? 'rgba(0, 0, 0, 0.4)' : 'rgba(30, 32, 28, 0.06)';
          ctx.shadowBlur = 8;
          ctx.shadowOffsetY = 2;

          // Card Background
          drawRoundRect(ctx, cardX, cardY, cardW, cardH, cardR);
          ctx.fillStyle = isDark
            ? (node.kind === 'object' ? '#20221E' : '#1C1D19')
            : (node.kind === 'object' ? '#FAF9FE' : '#FFFFFF');
          ctx.fill();

          // Reset shadow
          ctx.shadowColor = 'transparent';

          // Card Border
          ctx.strokeStyle = isSelected
            ? (isDark ? '#879E78' : '#49583D')
            : isHovered
            ? (isDark ? '#4F5247' : '#B8B4A7')
            : node.kind === 'object'
            ? (node.color || '#D8D2EB')
            : (isDark ? '#2C2E27' : '#E6E2D8');
          ctx.lineWidth = isSelected ? 2 : 1;
          ctx.stroke();

          // Left Icon Box
          const iconBoxSize = 26;
          const iconBoxX = cardX + 8;
          const iconBoxY = cardY + (cardH - iconBoxSize) / 2;
          drawRoundRect(ctx, iconBoxX, iconBoxY, iconBoxSize, iconBoxSize, 7);
          ctx.fillStyle = isDark
            ? '#262923'
            : node.kind === 'object'
            ? '#F0ECFA'
            : '#F2F6FA';
          ctx.fill();

          // Icon
          ctx.font = '13px Apple Color Emoji, Segoe UI Emoji, system-ui';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          ctx.fillText(
            node.icon || (node.kind === 'object' ? '⚖️' : '📄'),
            iconBoxX + iconBoxSize / 2,
            iconBoxY + iconBoxSize / 2
          );

          // Note / Object Label
          ctx.font = '12px Inter, system-ui, sans-serif';
          ctx.textAlign = 'left';
          ctx.textBaseline = 'middle';
          ctx.fillStyle = isDark ? '#EDEAE2' : '#1E201C';

          const maxTextW = cardW - iconBoxSize - 28;
          let displayLabel = node.label;
          if (ctx.measureText(displayLabel).width > maxTextW) {
            while (displayLabel.length > 3 && ctx.measureText(displayLabel + '…').width > maxTextW) {
              displayLabel = displayLabel.slice(0, -1);
            }
            displayLabel += '…';
          }
          ctx.fillText(displayLabel, cardX + iconBoxSize + 15, cardY + cardH / 2);

          // Draft status amber dot
          if (node.draft_status) {
            ctx.beginPath();
            ctx.arc(cardX + cardW - 8, cardY + 8, 3.5, 0, 2 * Math.PI);
            ctx.fillStyle = '#E8982C';
            ctx.fill();
            ctx.strokeStyle = isDark ? '#1C1D19' : '#FFFFFF';
            ctx.lineWidth = 1.5;
            ctx.stroke();
          }
        }

        ctx.restore();
      }

      ctx.restore();

      // --- Render Radar Minimap ---
      const minimapCanvas = minimapCanvasRef.current;
      if (minimapCanvas && nodes.length > 0) {
        const mctx = minimapCanvas.getContext('2d');
        if (mctx) {
          const mw = minimapCanvas.width;
          const mh = minimapCanvas.height;
          mctx.clearRect(0, 0, mw, mh);

          // Find bounding box of all nodes
          let minX = Infinity;
          let maxX = -Infinity;
          let minY = Infinity;
          let maxY = -Infinity;

          for (const n of nodes) {
            if (n.x < minX) minX = n.x;
            if (n.x > maxX) maxX = n.x;
            if (n.y < minY) minY = n.y;
            if (n.y > maxY) maxY = n.y;
          }

          const pad = 120;
          minX -= pad;
          maxX += pad;
          minY -= pad;
          maxY += pad;

          const spanX = Math.max(100, maxX - minX);
          const spanY = Math.max(100, maxY - minY);

          const scaleX = mw / spanX;
          const scaleY = mh / spanY;
          const mScale = Math.min(scaleX, scaleY);

          // Minimap background
          mctx.fillStyle = isDark ? '#1C1D19' : '#FAF8F3';
          mctx.fillRect(0, 0, mw, mh);

          // Draw node dots
          for (const n of nodes) {
            const mx = (n.x - minX) * mScale;
            const my = (n.y - minY) * mScale;

            mctx.beginPath();
            mctx.arc(mx, my, n.kind === 'project' ? 3.5 : 2, 0, 2 * Math.PI);
            mctx.fillStyle = n.kind === 'project'
              ? (isDark ? '#879E78' : '#384232')
              : (isDark ? '#B1B5AB' : '#8C8A82');
            mctx.fill();
          }

          // Draw current viewport rectangle
          const vpLeft = (-transform.x / transform.scale - minX) * mScale;
          const vpTop = (-transform.y / transform.scale - minY) * mScale;
          const vpW = (rectWidth / transform.scale) * mScale;
          const vpH = (rectHeight / transform.scale) * mScale;

          mctx.strokeStyle = isDark ? 'rgba(135, 158, 120, 0.6)' : 'rgba(73, 88, 61, 0.5)';
          mctx.lineWidth = 1.2;
          mctx.strokeRect(vpLeft, vpTop, vpW, vpH);
        }
      }

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
        if (node.kind === 'project') {
          const dx = node.x - canvasX;
          const dy = node.y - canvasY;
          if (dx * dx + dy * dy <= (node.radius + 8) * (node.radius + 8)) {
            return node;
          }
        } else {
          const w = node.cardWidth || 170;
          const h = node.cardHeight || 42;
          if (
            Math.abs(canvasX - node.x) <= w / 2 &&
            Math.abs(canvasY - node.y) <= h / 2
          ) {
            return node;
          }
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
    const zoomFactor = e.deltaY < 0 ? 1.08 : 0.92;
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const currentScale = transformRef.current.scale;
    const nextScale = Math.min(2.5, Math.max(0.35, currentScale * zoomFactor));

    transformRef.current.x = mouseX - (mouseX - transformRef.current.x) * (nextScale / currentScale);
    transformRef.current.y = mouseY - (mouseY - transformRef.current.y) * (nextScale / currentScale);
    transformRef.current.scale = nextScale;
    setZoomScale(nextScale);
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
    setZoomScale(1);
  };

  const zoomIn = () => {
    const next = Math.min(2.5, transformRef.current.scale * 1.25);
    transformRef.current.scale = next;
    setZoomScale(next);
  };

  const zoomOut = () => {
    const next = Math.max(0.35, transformRef.current.scale * 0.8);
    transformRef.current.scale = next;
    setZoomScale(next);
  };

  const fitToView = () => {
    const canvas = canvasRef.current;
    const nodes = simNodesRef.current;
    if (!canvas || nodes.length === 0) {
      resetZoom();
      return;
    }

    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;

    for (const n of nodes) {
      if (n.x < minX) minX = n.x;
      if (n.x > maxX) maxX = n.x;
      if (n.y < minY) minY = n.y;
      if (n.y > maxY) maxY = n.y;
    }

    const pad = 80;
    const spanW = Math.max(100, maxX - minX + pad * 2);
    const spanH = Math.max(100, maxY - minY + pad * 2);

    const rect = canvas.getBoundingClientRect();
    const scale = Math.min(2.0, Math.max(0.4, Math.min(rect.width / spanW, rect.height / spanH)));

    const midX = (minX + maxX) / 2;
    const midY = (minY + maxY) / 2;

    transformRef.current = {
      x: rect.width / 2 - midX * scale,
      y: rect.height / 2 - midY * scale,
      scale,
    };
    setZoomScale(scale);
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
      className="graph-container"
    >
      {/* 1. Generous Editorial Header */}
      <header className="graph-header">
        {/* Mountain watercolor silhouette background */}
        <div className="graph-header-art" aria-hidden="true">
          <svg viewBox="0 0 640 100" fill="none" preserveAspectRatio="none" className="w-full h-full opacity-35 dark:opacity-15">
            <path d="M0 100 Q120 62 240 76 T480 50 T640 65 L640 100 Z" fill="#9FB096" />
            <path d="M140 100 Q260 38 380 54 T640 28 L640 100 Z" fill="#667A58" />
            <path d="M260 100 Q400 16 520 28 T640 10 L640 100 Z" fill="#49583D" />
          </svg>
        </div>

        <div className="graph-header-top">
          <div>
            <div className="graph-eyebrow">Explore Your Knowledge</div>
            <div className="graph-title-row">
              <h1 className="graph-title">Knowledge Graph</h1>
              <div className="graph-stats-chip">
                <Share2 className="w-3.5 h-3.5 text-sthira-primary" />
                <span data-testid="graph-node-count" className="font-semibold text-sthira-text">
                  {totalVisibleNodes}
                </span>{' '}
                nodes
                <span className="text-sthira-text-muted">·</span>
                <span data-testid="graph-edge-count" className="font-semibold text-sthira-text">
                  {totalVisibleEdges}
                </span>{' '}
                edges
              </div>
            </div>
            <p className="graph-subtitle">
              A connected view of your projects, pages, objects and ideas.
            </p>
          </div>
        </div>
      </header>

      {/* 2. Secondary Filter & Search Toolbar */}
      <div className="graph-toolbar">
        {/* Left Filter Pills */}
        <div className="graph-filters-group">
          <span className="graph-filters-label">
            <SlidersHorizontal className="w-3.5 h-3.5" />
            Filters:
          </span>

          <button
            type="button"
            data-testid="filter-projects"
            onClick={() => setShowProjects((prev) => !prev)}
            className={`graph-filter-pill ${showProjects ? 'active-primary' : 'inactive'}`}
          >
            <span>📁</span>
            <span>Projects</span>
            {showProjects && <Check className="w-3 h-3 check-icon" />}
          </button>

          <button
            type="button"
            data-testid="filter-pages"
            onClick={() => setShowPages((prev) => !prev)}
            className={`graph-filter-pill ${showPages ? 'active-secondary' : 'inactive'}`}
          >
            <span>📄</span>
            <span>Pages</span>
            {showPages && <Check className="w-3 h-3 check-icon" />}
          </button>

          <button
            type="button"
            data-testid="filter-ideas"
            onClick={() => setShowIdeas((prev) => !prev)}
            className={`graph-filter-pill ${showIdeas ? 'active-secondary' : 'inactive'}`}
          >
            <span>💡</span>
            <span>Ideas</span>
            {showIdeas && <Check className="w-3 h-3 check-icon" />}
          </button>

          <button
            type="button"
            data-testid="filter-tasks"
            onClick={() => setShowTasks((prev) => !prev)}
            className={`graph-filter-pill ${showTasks ? 'active-secondary' : 'inactive'}`}
          >
            <span>☑️</span>
            <span>Tasks</span>
            {showTasks && <Check className="w-3 h-3 check-icon" />}
          </button>

          <button
            type="button"
            data-testid="filter-sources"
            onClick={() => setShowSources((prev) => !prev)}
            className={`graph-filter-pill ${showSources ? 'active-secondary' : 'inactive'}`}
          >
            <span>🔗</span>
            <span>Sources</span>
            {showSources && <Check className="w-3 h-3 check-icon" />}
          </button>

          {/* Registered Custom Object Types (e.g. Decisions) */}
          {(objectTypes || [])
            .filter((ot) => ot && !['idea', 'todo', 'task'].includes((ot.id || '').toLowerCase()))
            .map((ot) => {
              const isActive = selectedCustomTypes[ot.id.toLowerCase()] ?? true;
              return (
                <button
                  key={ot.id}
                  type="button"
                  onClick={() =>
                    setSelectedCustomTypes((prev) => ({
                      ...prev,
                      [ot.id.toLowerCase()]: !isActive,
                    }))
                  }
                  className={`graph-filter-pill ${isActive ? 'active-secondary' : 'inactive'}`}
                >
                  <span>{ot.icon || '⚖️'}</span>
                  <span>{ot.display_name}</span>
                  {isActive && <Check className="w-3 h-3 check-icon" />}
                </button>
              );
            })}
        </div>

        {/* Right Search & Controls */}
        <div className="graph-actions-group">
          <div className="graph-search-container">
            <Search className="graph-search-icon" />
            <input
              ref={searchInputRef}
              type="text"
              data-testid="graph-search-input"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search nodes..."
              className="graph-search-input"
            />
            <span className="graph-search-shortcut">⌘F</span>
          </div>

          <button
            type="button"
            onClick={fitToView}
            title="Fullscreen View"
            className="graph-btn-icon"
          >
            <Maximize2 className="w-4 h-4" />
          </button>

          <button
            type="button"
            onClick={resetZoom}
            title="Reset View"
            className="graph-btn-icon"
          >
            <RotateCcw className="w-4 h-4" />
          </button>

          <button
            type="button"
            title="More Options"
            className="graph-btn-icon"
          >
            <MoreHorizontal className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* 3. Main Interactive Canvas Viewport */}
      <div className="graph-canvas-wrapper">
        {isLoading ? (
          <div className="absolute inset-0 flex items-center justify-center bg-sthira-canvas/80 z-10 text-xs text-sthira-text-muted">
            Loading knowledge graph...
          </div>
        ) : error ? (
          <div className="absolute inset-0 flex items-center justify-center bg-sthira-canvas z-10 p-6">
            <div className="p-4 rounded-xl bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900 text-red-700 dark:text-red-300 text-xs max-w-md">
              {error}
            </div>
          </div>
        ) : totalVisibleNodes === 0 ? (
          <div className="absolute inset-0 flex flex-col items-center justify-center text-center p-6 text-sthira-text-muted">
            <Share2 className="w-12 h-12 mb-3 stroke-1 text-sthira-primary" />
            <h3 className="text-sm font-semibold text-sthira-text">No matching graph nodes</h3>
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
          className="graph-canvas"
        />

        {/* Floating Selected Node Details Card */}
        {selectedNode && (
          <div className="graph-selected-card animate-in fade-in slide-in-from-top-2 duration-150">
            <div className="flex items-start justify-between gap-2 mb-2">
              <div className="flex items-center gap-2">
                <span className="text-xl">{selectedNode.icon || (selectedNode.kind === 'project' ? '📁' : '📄')}</span>
                <div>
                  <h4 className="font-bold text-sthira-text line-clamp-1">{selectedNode.label}</h4>
                  <span className="text-[10px] uppercase font-semibold text-sthira-text-muted tracking-wider">
                    {selectedNode.kind === 'object'
                      ? `${selectedNode.object_type || 'object'}`
                      : selectedNode.kind}
                  </span>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setSelectedNode(null)}
                className="text-sthira-text-muted hover:text-sthira-text p-0.5 rounded cursor-pointer"
              >
                ✕
              </button>
            </div>

            {selectedNode.kind === 'project' && selectedNode.item_count !== undefined && (
              <p className="text-sthira-text-muted text-[11px] mb-3">
                This project contains <strong>{selectedNode.item_count}</strong> attached pages and entities.
              </p>
            )}

            {selectedNode.kind !== 'project' && (
              <p className="text-sthira-text-muted text-[11px] mb-3">
                {selectedNode.draft_status
                  ? 'Unfinished draft note ready for continuation.'
                  : 'Synthesized knowledge artifact in workspace.'}
              </p>
            )}

            <button
              type="button"
              data-testid="btn-open-selected-node"
              onClick={() => handleNavigateToNode(selectedNode)}
              className="w-full py-1.5 px-3 rounded-xl bg-sthira-primary hover:bg-sthira-primary-hover text-sthira-on-primary font-semibold text-xs flex items-center justify-center gap-1.5 transition-colors cursor-pointer shadow-xs"
            >
              <span>{selectedNode.kind === 'project' ? 'Open Project' : 'Focus Note'}</span>
              <ExternalLink className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {/* Floating Legend Card (Bottom-Left) */}
        <div className="graph-legend-card hidden sm:block">
          <div className="graph-legend-header">
            <span>Legend</span>
            <Info className="w-3.5 h-3.5 text-sthira-text-muted" />
          </div>
          <div className="graph-legend-row">
            <span className="graph-legend-badge">
              <span>📁</span> Project
            </span>
            <span className="graph-legend-desc">
              A workspace for focused work (e.g., research, build, plan)
            </span>
          </div>
          <div className="graph-legend-row">
            <span className="graph-legend-badge">
              <span>📄</span> Page / Note
            </span>
            <span className="graph-legend-desc">
              Knowledge, research, and content
            </span>
          </div>
          <div className="graph-legend-row">
            <span className="graph-legend-badge">
              <span>⚖️</span> Object
            </span>
            <span className="graph-legend-desc">
              Structured data, decisions, or custom entities
            </span>
          </div>
        </div>

        {/* Viewport Zoom & Minimap Floating Toolbar (Bottom-Right) */}
        <div className="graph-viewport-toolbar">
          <div className="graph-zoom-bar">
            <button
              type="button"
              onClick={zoomOut}
              title="Zoom Out"
              className="graph-zoom-btn"
            >
              −
            </button>
            <span
              onClick={resetZoom}
              title="Reset to 100%"
              className="graph-zoom-text"
            >
              {Math.round(zoomScale * 100)}%
            </span>
            <button
              type="button"
              onClick={zoomIn}
              title="Zoom In"
              className="graph-zoom-btn"
            >
              +
            </button>
          </div>

          <button
            type="button"
            onClick={fitToView}
            title="Fit to View"
            className="graph-fit-btn"
          >
            <Maximize2 className="w-4 h-4" />
          </button>

          {/* Minimap Preview Card */}
          <div
            className="graph-minimap"
            title="Minimap Radar View"
            onClick={fitToView}
          >
            <canvas
              ref={minimapCanvasRef}
              width={100}
              height={68}
            />
          </div>
        </div>
      </div>
    </div>
  );
};

export default GraphView;
