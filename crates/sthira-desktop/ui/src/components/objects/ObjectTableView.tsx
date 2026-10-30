import React, { useState, useMemo } from 'react';
import { ArrowUpDown, ArrowUp, ArrowDown, Search, ExternalLink } from 'lucide-react';
import { PropertyFieldRenderer } from './PropertyFieldRenderer';
import type { ObjectType, ObjectItem } from '../../types/sthira';

interface ObjectTableViewProps {
  objectType: ObjectType;
  items: ObjectItem[];
  onUpdateProperty: (artifactId: string, propertyKey: string, newValue: any) => void;
  onOpenItem: (artifactId: string, projectId?: string | null) => void;
}

export const ObjectTableView: React.FC<ObjectTableViewProps> = ({
  objectType,
  items,
  onUpdateProperty,
  onOpenItem,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [sortField, setSortField] = useState<string>('updated_at');
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('desc');

  const handleSort = (fieldKey: string) => {
    if (sortField === fieldKey) {
      setSortDirection((prev) => (prev === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortField(fieldKey);
      setSortDirection('asc');
    }
  };

  const filteredAndSortedItems = useMemo(() => {
    let result = [...items];

    // Search filter
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase().trim();
      result = result.filter((item) => {
        if (item.title.toLowerCase().includes(q)) return true;
        for (const [_, val] of Object.entries(item.properties)) {
          if (val && String(val).toLowerCase().includes(q)) return true;
        }
        return false;
      });
    }

    // Sort
    result.sort((a, b) => {
      let valA: any = sortField === 'title' ? a.title : sortField === 'updated_at' ? a.updated_at : a.properties[sortField];
      let valB: any = sortField === 'title' ? b.title : sortField === 'updated_at' ? b.updated_at : b.properties[sortField];

      if (valA === undefined || valA === null) valA = '';
      if (valB === undefined || valB === null) valB = '';

      if (typeof valA === 'number' && typeof valB === 'number') {
        return sortDirection === 'asc' ? valA - valB : valB - valA;
      }

      const strA = String(valA).toLowerCase();
      const strB = String(valB).toLowerCase();
      if (strA < strB) return sortDirection === 'asc' ? -1 : 1;
      if (strA > strB) return sortDirection === 'asc' ? 1 : -1;
      return 0;
    });

    return result;
  }, [items, searchQuery, sortField, sortDirection]);

  const renderSortIndicator = (fieldKey: string) => {
    if (sortField !== fieldKey) {
      return <ArrowUpDown className="w-3 h-3 text-[#B0ACA2] opacity-0 group-hover:opacity-100 transition-opacity" />;
    }
    return sortDirection === 'asc' ? (
      <ArrowUp className="w-3 h-3 text-[#282C25]" />
    ) : (
      <ArrowDown className="w-3 h-3 text-[#282C25]" />
    );
  };

  const formatDate = (ts: number) => {
    if (!ts) return '—';
    const date = new Date(ts);
    return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
  };

  return (
    <div className="flex flex-col h-full bg-[#FAF9F5] rounded-2xl border border-[#E8E5DC] overflow-hidden shadow-2xs">
      {/* Table Toolbar */}
      <div className="px-4 py-3 bg-[#F5F3EC]/60 border-b border-[#E8E5DC] flex items-center justify-between gap-3">
        <div className="relative flex-1 max-w-sm">
          <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-[#8C8A82]" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={`Search ${objectType.display_name.toLowerCase()}s...`}
            className="w-full pl-8 pr-3 py-1.5 rounded-xl border border-[#E8E5DC] bg-white text-xs outline-none focus:border-[#282C25] transition-colors"
          />
        </div>

        <div className="text-xs text-[#8C8A82]">
          Showing <span className="font-semibold text-[#1E201C]">{filteredAndSortedItems.length}</span> of {items.length} {objectType.display_name.toLowerCase()}s
        </div>
      </div>

      {/* Table Body */}
      <div className="flex-1 overflow-auto">
        <table className="w-full text-left text-xs border-collapse">
          <thead>
            <tr className="border-b border-[#E8E5DC] bg-[#EFECE4]/70 text-[#6E6D66] font-semibold sticky top-0 z-10 backdrop-blur-xs">
              {/* Title column */}
              <th
                onClick={() => handleSort('title')}
                className="px-4 py-3 cursor-pointer group hover:text-[#1E201C] transition-colors min-w-[200px]"
              >
                <div className="flex items-center gap-1.5">
                  <span>Name</span>
                  {renderSortIndicator('title')}
                </div>
              </th>

              {/* Dynamic Property columns */}
              {objectType.schema.map((prop) => (
                <th
                  key={prop.key}
                  onClick={() => handleSort(prop.key)}
                  className="px-4 py-3 cursor-pointer group hover:text-[#1E201C] transition-colors min-w-[140px]"
                >
                  <div className="flex items-center gap-1.5">
                    <span>{prop.label}</span>
                    {renderSortIndicator(prop.key)}
                  </div>
                </th>
              ))}

              {/* Updated At */}
              <th
                onClick={() => handleSort('updated_at')}
                className="px-4 py-3 cursor-pointer group hover:text-[#1E201C] transition-colors w-32"
              >
                <div className="flex items-center gap-1.5">
                  <span>Updated</span>
                  {renderSortIndicator('updated_at')}
                </div>
              </th>

              {/* Actions */}
              <th className="px-3 py-3 w-16 text-center">Action</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[#E8E5DC]/80">
            {filteredAndSortedItems.length === 0 ? (
              <tr>
                <td
                  colSpan={objectType.schema.length + 3}
                  className="px-4 py-12 text-center text-xs text-[#8C8A82] italic"
                >
                  {items.length === 0
                    ? `No ${objectType.display_name.toLowerCase()}s found. Click "+ New ${objectType.display_name}" above to create one.`
                    : 'No matching items match your search.'}
                </td>
              </tr>
            ) : (
              filteredAndSortedItems.map((item) => (
                <tr
                  key={item.artifact_id}
                  className="hover:bg-white transition-colors group"
                >
                  {/* Title / Name */}
                  <td className="px-4 py-2.5 font-medium text-[#1E201C]">
                    <div
                      onClick={() => onOpenItem(item.artifact_id, item.project_id)}
                      className="cursor-pointer hover:underline flex items-center gap-2 group-hover:text-[#282C25]"
                    >
                      <span>{objectType.icon}</span>
                      <span className="truncate max-w-[220px]">{item.title || 'Untitled'}</span>
                    </div>
                  </td>

                  {/* Schema fields */}
                  {objectType.schema.map((prop) => (
                    <td key={prop.key} className="px-4 py-2.5">
                      <PropertyFieldRenderer
                        definition={prop}
                        value={item.properties[prop.key]}
                        compact={true}
                        onChange={(newVal) =>
                          onUpdateProperty(item.artifact_id, prop.key, newVal)
                        }
                      />
                    </td>
                  ))}

                  {/* Updated At */}
                  <td className="px-4 py-2.5 text-xs text-[#8C8A82]">
                    {formatDate(item.updated_at)}
                  </td>

                  {/* Action Link */}
                  <td className="px-3 py-2.5 text-center">
                    <button
                      type="button"
                      onClick={() => onOpenItem(item.artifact_id, item.project_id)}
                      className="p-1 rounded text-[#8C8A82] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors cursor-pointer"
                      title="Open note in Focus View"
                    >
                      <ExternalLink className="w-3.5 h-3.5" />
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
};

export default ObjectTableView;
