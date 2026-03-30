"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { fetchApi } from "@/lib/api";

interface DailyBusinessMetrics {
  date: string;
  revenue_gaming_paise: number;
  revenue_cafe_paise: number;
  revenue_other_paise: number;
  expense_rent_paise: number;
  expense_utilities_paise: number;
  expense_salaries_paise: number;
  expense_maintenance_paise: number;
  expense_other_paise: number;
  sessions_count: number;
  occupancy_rate_pct: number;
}

const formatINR = (paise: number) =>
  "\u20B9" +
  (paise / 100).toLocaleString("en-IN", { minimumFractionDigits: 2 });

function totalRevenue(d: DailyBusinessMetrics): number {
  return d.revenue_gaming_paise + d.revenue_cafe_paise + d.revenue_other_paise;
}

function totalExpenses(d: DailyBusinessMetrics): number {
  return (
    d.expense_rent_paise +
    d.expense_utilities_paise +
    d.expense_salaries_paise +
    d.expense_maintenance_paise +
    d.expense_other_paise
  );
}

function ebitda(d: DailyBusinessMetrics): number {
  return totalRevenue(d) - totalExpenses(d);
}

export default function EbitdaPage() {
  const [data, setData] = useState<DailyBusinessMetrics[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const today = new Date();
    const start = new Date(today);
    start.setDate(1);
    const startStr = start.toISOString().split("T")[0];
    const endStr = today.toISOString().split("T")[0];

    fetchApi<{ days: DailyBusinessMetrics[] }>(
      `/analytics/business?start=${startStr}&end=${endStr}`
    )
      .then((res) => {
        setData(res.days || []);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  const sumRevenue = data.reduce((s, d) => s + totalRevenue(d), 0);
  const sumExpenses = data.reduce((s, d) => s + totalExpenses(d), 0);
  const sumEbitda = sumRevenue - sumExpenses;
  const avgDailyEbitda = data.length > 0 ? sumEbitda / data.length : 0;

  const sumGaming = data.reduce((s, d) => s + d.revenue_gaming_paise, 0);
  const sumCafe = data.reduce((s, d) => s + d.revenue_cafe_paise, 0);
  const sumOther = data.reduce((s, d) => s + d.revenue_other_paise, 0);
  const maxRevenue = Math.max(sumGaming, sumCafe, sumOther, 1);

  const expenseCategories = [
    { label: "Rent", value: data.reduce((s, d) => s + d.expense_rent_paise, 0) },
    { label: "Utilities", value: data.reduce((s, d) => s + d.expense_utilities_paise, 0) },
    { label: "Salaries", value: data.reduce((s, d) => s + d.expense_salaries_paise, 0) },
    { label: "Maintenance", value: data.reduce((s, d) => s + d.expense_maintenance_paise, 0) },
    { label: "Other", value: data.reduce((s, d) => s + d.expense_other_paise, 0) },
  ];
  const maxExpense = Math.max(...expenseCategories.map((e) => e.value), 1);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">EBITDA &amp; Financial Dashboard</h1>
          <p className="text-sm text-rp-grey">Monthly revenue, expenses, and profitability</p>
        </div>
        <span className="text-xs text-rp-grey">{data.length} days loaded</span>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading financial data...</div>
      ) : (
        <>
          {/* Summary Cards */}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
            <SummaryCard
              title="Total Revenue"
              value={formatINR(sumRevenue)}
              color="text-green-400"
              borderColor="border-green-500/30"
            />
            <SummaryCard
              title="Total Expenses"
              value={formatINR(sumExpenses)}
              color="text-red-400"
              borderColor="border-red-500/30"
            />
            <SummaryCard
              title="EBITDA"
              value={formatINR(sumEbitda)}
              color={sumEbitda >= 0 ? "text-blue-400" : "text-red-400"}
              borderColor={sumEbitda >= 0 ? "border-blue-500/30" : "border-red-500/30"}
            />
            <SummaryCard
              title="Avg Daily EBITDA"
              value={formatINR(avgDailyEbitda)}
              color={avgDailyEbitda >= 0 ? "text-blue-400" : "text-red-400"}
              borderColor={avgDailyEbitda >= 0 ? "border-blue-500/30" : "border-red-500/30"}
            />
          </div>

          {/* Revenue Breakdown */}
          <div className="bg-rp-card border border-rp-border rounded-lg p-6 mb-6">
            <h2 className="text-lg font-semibold text-white mb-4">Revenue Breakdown</h2>
            <div className="space-y-3">
              <BarRow label="Gaming" value={sumGaming} max={maxRevenue} color="bg-green-500" />
              <BarRow label="Cafe" value={sumCafe} max={maxRevenue} color="bg-emerald-400" />
              <BarRow label="Other" value={sumOther} max={maxRevenue} color="bg-teal-400" />
            </div>
          </div>

          {/* Expense Breakdown */}
          <div className="bg-rp-card border border-rp-border rounded-lg p-6 mb-6">
            <h2 className="text-lg font-semibold text-white mb-4">Expense Breakdown</h2>
            <div className="space-y-3">
              {expenseCategories.map((cat) => (
                <BarRow
                  key={cat.label}
                  label={cat.label}
                  value={cat.value}
                  max={maxExpense}
                  color="bg-red-500"
                />
              ))}
            </div>
          </div>

          {/* Daily Trend Table */}
          <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
            <div className="px-6 py-4 border-b border-rp-border">
              <h2 className="text-lg font-semibold text-white">Daily Trend</h2>
            </div>
            {data.length === 0 ? (
              <div className="p-8 text-center text-rp-grey text-sm">No data available</div>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b border-rp-border text-rp-grey text-left">
                      <th className="px-4 py-3 font-medium">Date</th>
                      <th className="px-4 py-3 font-medium text-right">Revenue</th>
                      <th className="px-4 py-3 font-medium text-right">Expenses</th>
                      <th className="px-4 py-3 font-medium text-right">EBITDA</th>
                      <th className="px-4 py-3 font-medium text-right">Occupancy</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.map((day) => {
                      const dayEbitda = ebitda(day);
                      return (
                        <tr
                          key={day.date}
                          className="border-b border-rp-border/50 hover:bg-white/5"
                        >
                          <td className="px-4 py-3 text-neutral-300">{day.date}</td>
                          <td className="px-4 py-3 text-right text-green-400">
                            {formatINR(totalRevenue(day))}
                          </td>
                          <td className="px-4 py-3 text-right text-red-400">
                            {formatINR(totalExpenses(day))}
                          </td>
                          <td
                            className={`px-4 py-3 text-right font-medium ${
                              dayEbitda >= 0 ? "text-green-400" : "text-red-400"
                            }`}
                          >
                            {formatINR(dayEbitda)}
                          </td>
                          <td className="px-4 py-3 text-right text-neutral-300">
                            {day.occupancy_rate_pct.toFixed(1)}%
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </>
      )}
    </DashboardLayout>
  );
}

function SummaryCard({
  title,
  value,
  color,
  borderColor,
}: {
  title: string;
  value: string;
  color: string;
  borderColor: string;
}) {
  return (
    <div className={`bg-rp-card border ${borderColor} rounded-lg p-5`}>
      <p className="text-xs text-rp-grey mb-1">{title}</p>
      <p className={`text-xl font-bold ${color}`}>{value}</p>
    </div>
  );
}

function BarRow({
  label,
  value,
  max,
  color,
}: {
  label: string;
  value: number;
  max: number;
  color: string;
}) {
  const pct = max > 0 ? (value / max) * 100 : 0;
  return (
    <div className="flex items-center gap-3">
      <span className="text-sm text-neutral-300 w-28 shrink-0">{label}</span>
      <div className="flex-1 bg-rp-border/50 rounded-full h-5 overflow-hidden">
        <div
          className={`${color} h-full rounded-full transition-all`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-sm text-neutral-400 w-32 text-right shrink-0">
        {formatINR(value)}
      </span>
    </div>
  );
}
