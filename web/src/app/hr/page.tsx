"use client";

import DashboardLayout from "@/components/DashboardLayout";

const HR_SECTIONS = [
  {
    title: "Employees",
    description: "Staff profiles, roles, and contact information",
    href: "/hr/employees",
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z" />
      </svg>
    ),
  },
  {
    title: "Shifts",
    description: "Shift schedules and attendance tracking",
    href: "/hr/shifts",
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    ),
  },
  {
    title: "Leaderboard",
    description: "Staff maintenance performance rankings",
    href: "/hr/leaderboard",
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M16.5 18.75h-9m9 0a3 3 0 013 3h-15a3 3 0 013-3m9 0v-3.375c0-.621-.503-1.125-1.125-1.125h-.871M7.5 18.75v-3.375c0-.621.504-1.125 1.125-1.125h.872m5.007 0H9.497m5.007 0a7.454 7.454 0 01-.982-3.172M9.497 14.25a7.454 7.454 0 00.981-3.172M5.25 4.236c-.982.143-1.954.317-2.916.52A6.003 6.003 0 007.73 9.728M5.25 4.236V4.5c0 2.108.966 3.99 2.48 5.228M5.25 4.236V2.721C7.456 2.41 9.71 2.25 12 2.25c2.291 0 4.545.16 6.75.47v1.516M18.75 4.236c.982.143 1.954.317 2.916.52A6.003 6.003 0 0016.27 9.728M18.75 4.236V4.5c0 2.108-.966 3.99-2.48 5.228m0 0a6.023 6.023 0 01-7.54 0" />
      </svg>
    ),
  },
];

export default function HRPage() {
  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white">Human Resources</h1>
        <p className="text-sm text-rp-grey">Staff management, scheduling, and performance</p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        {HR_SECTIONS.map((section) => (
          <a
            key={section.title}
            href={section.href}
            className="bg-rp-card border border-rp-border rounded-lg p-6 hover:border-neutral-500 transition-colors group"
          >
            <div className="text-rp-grey group-hover:text-white transition-colors mb-3">
              {section.icon}
            </div>
            <h2 className="text-lg font-semibold text-white mb-1">{section.title}</h2>
            <p className="text-sm text-rp-grey">{section.description}</p>
          </a>
        ))}
      </div>
    </DashboardLayout>
  );
}
