"use client";

import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  flexRender,
  type ColumnDef,
  type SortingState,
} from "@tanstack/react-table";
import { useState } from "react";
import { SkeletonRow, EmptyState } from "./Skeleton";

interface LiveDataTableProps<T> {
  data: T[];
  columns: ColumnDef<T, unknown>[];
  loading?: boolean;
  emptyIcon?: React.ReactNode;
  emptyHeadline?: string;
  emptyHint?: string;
  onRowSelect?: (row: T) => void;
  getRowId?: (row: T) => string;
}

export function LiveDataTable<T>({
  data,
  columns,
  loading,
  emptyIcon,
  emptyHeadline,
  emptyHint,
  onRowSelect,
  getRowId,
}: LiveDataTableProps<T>) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [selectedRowId, setSelectedRowId] = useState<string | null>(null);

  const table = useReactTable({
    data,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getRowId,
  });

  // Loading state: 5 skeleton rows
  if (loading) {
    return (
      <div className="overflow-auto rounded-lg border border-rp-border">
        <table className="w-full text-sm" style={{ minWidth: "max-content" }}>
          <thead className="sticky top-0 z-10 bg-rp-black">
            <tr className="bg-rp-black border-b border-rp-border">
              {table.getHeaderGroups().map((hg) =>
                hg.headers.map((header) => (
                  <th
                    key={header.id}
                    className="px-4 py-2 text-left text-xs font-medium text-rp-grey uppercase tracking-wider"
                  >
                    {header.isPlaceholder
                      ? null
                      : flexRender(header.column.columnDef.header, header.getContext())}
                  </th>
                ))
              )}
            </tr>
          </thead>
          <tbody>
            {Array.from({ length: 5 }).map((_, i) => (
              <tr key={i}>
                <td colSpan={columns.length}>
                  <SkeletonRow />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  }

  // Empty state
  if (data.length === 0) {
    return (
      <div className="overflow-auto rounded-lg border border-rp-border">
        <EmptyState
          icon={emptyIcon}
          headline={emptyHeadline ?? "No data"}
          hint={emptyHint}
        />
      </div>
    );
  }

  return (
    <div className="overflow-auto rounded-lg border border-rp-border">
      <table className="w-full text-sm" style={{ minWidth: "max-content" }}>
        <thead className="sticky top-0 z-10 bg-rp-black">
          {table.getHeaderGroups().map((headerGroup) => (
            <tr
              key={headerGroup.id}
              className="bg-rp-black border-b border-rp-border"
            >
              {headerGroup.headers.map((header) => {
                const canSort = header.column.getCanSort();
                const sorted = header.column.getIsSorted();
                return (
                  <th
                    key={header.id}
                    className={`px-4 py-2 text-left text-xs font-medium text-rp-grey uppercase tracking-wider ${
                      canSort
                        ? "cursor-pointer select-none hover:text-white"
                        : ""
                    }`}
                    onClick={
                      canSort
                        ? header.column.getToggleSortingHandler()
                        : undefined
                    }
                  >
                    {header.isPlaceholder
                      ? null
                      : flexRender(
                          header.column.columnDef.header,
                          header.getContext()
                        )}
                    {canSort && (
                      <span className="ml-1">
                        {sorted === "asc"
                          ? "\u25B2"
                          : sorted === "desc"
                            ? "\u25BC"
                            : "\u2B0D"}
                      </span>
                    )}
                  </th>
                );
              })}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => {
            const isSelected = selectedRowId === row.id;
            return (
              <tr
                key={row.id}
                onClick={() => {
                  setSelectedRowId(row.id);
                  onRowSelect?.(row.original);
                }}
                className={
                  isSelected
                    ? "bg-rp-red/10 border-b border-rp-border/50 cursor-pointer"
                    : "border-b border-rp-border/50 hover:bg-rp-card transition-colors cursor-pointer"
                }
              >
                {row.getVisibleCells().map((cell) => (
                  <td key={cell.id} className="px-4 py-3 text-neutral-200">
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
