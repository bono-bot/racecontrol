import Link from "next/link";
import { ArrowLeft, Wallet, History, CreditCard, Phone, Mail } from "lucide-react";
import { Panel, ActionButton, StatusTile, StatusDot } from "@/components/rp";

// Customer Detail — POS profile preview after CIRS lookup found.
// MOCK data; Phase 1 wires GET /api/v1/customers/[id] + wallet + sessions.

const MOCK_CUSTOMER = {
  id: "cus_01ja",
  name: "Vivek Singh",
  phone: "+91 98765 43210",
  email: "vivek@example.in",
  walletPaise: 270000, // ₹2,700.00
  loyaltyTier: "Apex" as const,
  totalSessions: 47,
  joinedAt: "2025-08-12",
  lastVisit: "today 17:42",
};

const MOCK_RECENT_SESSIONS = [
  { id: "ses_a1", date: "today 17:42", pod: 1, sim: "AC · Spa · SF15-T", duration: "30m", spendPaise: 30000 },
  { id: "ses_a2", date: "yesterday 21:15", pod: 5, sim: "F1 25 · Monaco", duration: "60m", spendPaise: 90000 },
  { id: "ses_a3", date: "2026-05-04 19:30", pod: 2, sim: "iRacing · Daytona", duration: "30m", spendPaise: 70000 },
];

function formatPaise(p: number) {
  return `₹${(p / 100).toLocaleString("en-IN", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

export default async function CustomerDetail({ params }: { params: Promise<{ id: string }> }) {
  await params; // TODO Phase 1: GET /api/v1/customers/<id>
  const c = MOCK_CUSTOMER;

  return (
    <div className="px-6 py-6 space-y-6">
      <div className="flex items-center gap-4">
        <Link href="/pos/lookup">
          <ActionButton variant="ghost" size="sm">
            <ArrowLeft className="size-4" /> Back
          </ActionButton>
        </Link>
        <div>
          <p className="font-body uppercase text-[11px] tracking-[0.12em] text-rp-grey">
            Customer · {c.id}
          </p>
          <h1 className="font-display text-3xl font-bold text-rp-ink uppercase tracking-tight">
            {c.name}
          </h1>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-1 space-y-6">
          <Panel title="Profile">
            <dl className="space-y-3 text-sm">
              <Field icon={<Phone className="size-3.5" />} label="Phone">
                <span className="font-mono text-rp-ink">{c.phone}</span>
              </Field>
              <Field icon={<Mail className="size-3.5" />} label="Email">
                <span className="font-body text-rp-ink-dim">{c.email}</span>
              </Field>
              <Field icon={<History className="size-3.5" />} label="Member since">
                <span className="font-mono text-rp-ink-dim">{c.joinedAt}</span>
              </Field>
              <Field label="Loyalty tier">
                <span className="inline-flex items-center gap-2">
                  <StatusDot state="amber" size={6} />
                  <span className="font-display uppercase text-rp-amber tracking-wide">{c.loyaltyTier}</span>
                </span>
              </Field>
              <Field label="Last visit">
                <span className="font-mono text-rp-ink-dim">{c.lastVisit}</span>
              </Field>
            </dl>
          </Panel>

          <Panel
            title="Wallet · sim/PS5 only"
            action={<span className="font-mono text-[10px] text-rp-ink-dim">Voucher · no expiry</span>}
          >
            <div className="space-y-4">
              <div>
                <p className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey mb-1">
                  Balance
                </p>
                <p className="font-mono text-4xl font-bold tabular-nums text-rp-ink">
                  {formatPaise(c.walletPaise)}
                </p>
                <p className="font-mono text-[10px] text-rp-ink-faint">
                  ≈ {Math.floor(c.walletPaise / 1500)} min @ tier-2 rate
                </p>
              </div>
              <div className="flex gap-2">
                <ActionButton variant="primary" size="md">
                  <Wallet className="size-4" /> Top-up
                </ActionButton>
                <ActionButton variant="secondary" size="md">
                  <CreditCard className="size-4" /> Start session
                </ActionButton>
              </div>
              <p className="font-mono text-[10px] text-rp-ink-faint pt-2 border-t border-rp-border">
                18% GST collected at top-up · cafe always paid separately
              </p>
            </div>
          </Panel>
        </div>

        <div className="col-span-2 space-y-6">
          <div className="grid grid-cols-3 gap-3">
            <StatusTile caption="Sessions" value={c.totalSessions} display />
            <StatusTile caption="Apex laps" value="138" sub="last 30 days" />
            <StatusTile caption="Best lap" value="1:48.951" sub="Spa · SF15-T" />
          </div>

          <Panel title="Recent sessions">
            <table className="w-full text-sm">
              <thead className="border-b border-rp-border">
                <tr className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey">
                  <th className="text-left pb-2 pr-4">When</th>
                  <th className="text-left pb-2 pr-4">Pod</th>
                  <th className="text-left pb-2 pr-4">Experience</th>
                  <th className="text-right pb-2 pr-4">Time</th>
                  <th className="text-right pb-2">Spend</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-rp-border">
                {MOCK_RECENT_SESSIONS.map((s) => (
                  <tr key={s.id} className="hover:bg-rp-card-hi transition-colors duration-(--motion-fast)">
                    <td className="py-2 pr-4 font-mono text-rp-ink-dim">{s.date}</td>
                    <td className="py-2 pr-4 font-display font-bold text-rp-ink">POD {s.pod}</td>
                    <td className="py-2 pr-4 font-body text-rp-ink-dim">{s.sim}</td>
                    <td className="py-2 pr-4 text-right font-mono tabular-nums text-rp-ink">{s.duration}</td>
                    <td className="py-2 text-right font-mono tabular-nums text-rp-ink">{formatPaise(s.spendPaise)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Panel>
        </div>
      </div>
    </div>
  );
}

function Field({ icon, label, children }: { icon?: React.ReactNode; label: string; children: React.ReactNode }) {
  return (
    <div>
      <dt className="flex items-center gap-1.5 font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey mb-0.5">
        {icon}
        {label}
      </dt>
      <dd className="text-sm">{children}</dd>
    </div>
  );
}
