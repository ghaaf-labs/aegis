"use client";

import { useEffect, useState } from "react";
import { DigestOptIn } from "@/components/settings/digest-opt-in";
import { DiaryVisibilityToggle } from "@/components/settings/diary-visibility-toggle";
import { portfolioApi } from "@/lib/api";
import { useApiQuery } from "@/lib/use-api-query";
import { useActivePortfolio } from "@/stores/portfolio";

export default function SettingsIndex() {
  const portfolio = useActivePortfolio();
  const portfolioId = portfolio?.id ?? "";

  const diaryQuery = useApiQuery(
    `portfolio.diaryPublic.${portfolioId}`,
    () => portfolioApi.getDiaryPublic(portfolioId),
    { enabled: !!portfolioId },
  );
  const [localDiaryPublic, setLocalDiaryPublic] = useState<boolean | null>(
    null,
  );
  const diaryPublic = localDiaryPublic ?? diaryQuery.data?.diaryPublic ?? false;

  const [storedEmail, setStoredEmail] = useState("");
  useEffect(() => {
    setStoredEmail(localStorage.getItem("aegis_email") ?? "");
  }, []);

  return (
    <div className="max-w-3xl mx-auto space-y-6">
      <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
        Settings
      </h1>

      <section>
        <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-2">
          Notifications
        </h2>
        <DigestOptIn defaultEmail={storedEmail} />
      </section>

      {portfolioId && (
        <section>
          <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-2">
            Privacy
          </h2>
          <DiaryVisibilityToggle
            key={`diary-${portfolioId}-${diaryPublic}`}
            initialPublic={diaryPublic}
            onChange={async (next) => {
              const res = await portfolioApi.setDiaryPublic(portfolioId, next);
              setLocalDiaryPublic(res.diaryPublic);
            }}
          />
        </section>
      )}
    </div>
  );
}
