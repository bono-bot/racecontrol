"use client";

import { useEffect, useRef, useState } from "react";
import { type ColumnDef } from "@tanstack/react-table";
import { ClipboardList } from "lucide-react";
import DashboardLayout from "@/components/DashboardLayout";
import StatusBadge from "@/components/StatusBadge";
import { LiveDataTable } from "@/components/LiveDataTable";
import { EmptyState } from "@/components/Skeleton";
import { useToast } from "@/components/Toast";
import type { Session } from "@/lib/api";
import { api } from "@/lib/api";

const simLabels: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  assetto_corsa_evo: "AC EVO",
  assetto_corsa_rally: "AC Rally",
  f1_25: "F1 25",
  iracing: "iRacing",
  le_mans_ultimate: "Le Mans Ultimate",
  forza: "Forza Motorsport",
  forza_horizon_5: "Forza Horizon 5",
};

const columns: ColumnDef<Session, unknown>[] = [
  {
    accessorKey: "pod_id",
    header: "Pod",
    size: 80,
    enableSorting: false,
    cell: ({ row }) => {
      // pod_id may be present in API response even if not in TS interface
      const val = "pod_id" in row.original
        ? (row.original as Session & { pod_id?: string | number }).pod_id
        : undefined;
      return (
        <span className="text-neutral-400 text-xs font-mono">
          {val != null ? String(val) : "\u2014"}
        </span>
      );
    },
  },
  {
    accessorKey: "type",
    header: "Type",
    enableSorting: true,
    cell: ({ getValue }) => (
      <span className="capitalize text-neutral-200">
        {String(getValue() ?? "")}
      </span>
    ),
  },
  {
    accessorKey: "track",
    header: "Track",
    enableSorting: true,
    cell: ({ getValue }) => (
      <span className="text-neutral-200 truncate max-w-[200px] block">
        {String(getValue() ?? "")}
      </span>
    ),
  },
  {
    accessorKey: "sim_type",
    header: "Sim",
    size: 130,
    enableSorting: false,
    cell: ({ getValue }) => {
      const raw = String(getValue() ?? "");
      return (
        <span className="text-neutral-300 text-xs">
          {simLabels[raw] || raw}
        </span>
      );
    },
  },
  {
    accessorKey: "car_class",
    header: "Car Class",
    size: 100,
    enableSorting: false,
    cell: ({ getValue }) => (
      <span className="text-neutral-400 text-xs">
        {String(getValue() ?? "") || "\u2014"}
      </span>
    ),
  },
  {
    accessorKey: "started_at",
    header: "Started",
    size: 160,
    enableSorting: true,
    cell: ({ getValue }) => {
      const raw = getValue();
      if (!raw) return <span className="text-rp-grey text-xs">{"\u2014"}</span>;
      return (
        <span className="text-rp-grey text-xs">
          {new Date(String(raw)).toLocaleString("en-IN", {
            timeZone: "Asia/Kolkata",
          })}
        </span>
      );
    },
  },
  {
    accessorKey: "status",
    header: "Status",
    size: 100,
    enableSorting: false,
    cell: ({ getValue }) => <StatusBadge status={String(getValue() ?? "")} />,
  },
];

export default function SessionsPage() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const hasFiredRef = useRef(false);
  const { toast } = useToast();

  useEffect(() => {
    api
      .listSessions()
      .then((res) => {
        const list = res.sessions || [];
        setSessions(list);
        setLoading(false);
        if (!hasFiredRef.current) {
          hasFiredRef.current = true;
          toast({ message: `Loaded ${list.length} sessions`, type: "success" });
        }
      })
      .catch(() => setLoading(false));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Sessions</h1>
          <p className="text-sm text-rp-grey">
            Practice, race, and qualifying sessions
          </p>
        </div>
        {!loading && (
          <span className="text-xs bg-rp-card border border-rp-border rounded-full px-3 py-1 text-rp-grey">
            {sessions.length} session{sessions.length !== 1 ? "s" : ""}
          </span>
        )}
      </div>

      {!loading && sessions.length === 0 ? (
        <div className="border border-rp-border rounded-lg">
          <EmptyState
            icon={<ClipboardList className="w-10 h-10" />}
            headline="No sessions yet"
            hint="Sessions are created when you start a practice, race, or qualifying run."
          />
        </div>
      ) : (
        <LiveDataTable<Session>
          data={sessions}
          columns={columns}
          loading={loading}
          emptyIcon={<ClipboardList className="w-10 h-10" />}
          emptyHeadline="No sessions yet"
          emptyHint="Sessions are created when you start a practice, race, or qualifying run."
          getRowId={(row) => row.id}
        />
      )}
    </DashboardLayout>
  );
}
