import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Layers3, Plus, Search, Trash2 } from "lucide-react";
import * as api from "../lib/tauri";
import type { BundleManifest, BundleStage, ManagedSkill } from "../lib/tauri";

const emptyBundle = (): BundleManifest => ({
  schema_version: 1, slug: "", description: "", instructions: "",
  stages: [{ id: "start", skills: [], after: [], when: null }],
});

export function Bundles() {
  const { t } = useTranslation();
  const [bundles, setBundles] = useState<BundleManifest[]>([]);
  const [skills, setSkills] = useState<ManagedSkill[]>([]);
  const [draft, setDraft] = useState<BundleManifest | null>(null);
  const [originalSlug, setOriginalSlug] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState("");
  const [skillQuery, setSkillQuery] = useState<Record<number, string>>({});

  const refresh = useCallback(async () => {
    const [nextBundles, nextSkills] = await Promise.all([api.listBundles(), api.getManagedSkills()]);
    setBundles(nextBundles);
    setSkills(nextSkills);
  }, []);
  useEffect(() => { void refresh().catch((error) => toast.error(String(error))); }, [refresh]);

  const deployed = (slug: string, agent: string) =>
    skills.some((skill) => skill.name === slug && skill.targets.some((target) => target.tool === agent));

  const edit = (bundle: BundleManifest) => {
    setDraft(structuredClone(bundle));
    setOriginalSlug(bundle.slug);
    setPreview("");
    setSkillQuery({});
  };
  const patchStage = (index: number, change: Partial<BundleStage>) => {
    if (!draft) return;
    setDraft({ ...draft, stages: draft.stages.map((stage, i) => i === index ? { ...stage, ...change } : stage) });
  };
  const execute = async (action: () => Promise<unknown>, success: string) => {
    setBusy(true);
    try {
      await action();
      await refresh();
      toast.success(success);
    } catch (error) {
      toast.error(String(error));
    } finally {
      setBusy(false);
    }
  };
  const save = () => {
    if (!draft) return;
    void execute(async () => {
      await api.saveBundle(draft, originalSlug);
      setDraft(null);
    }, t("bundles.saved"));
  };
  const remove = (slug: string) => {
    if (!window.confirm(t("bundles.confirmDelete", { slug }))) return;
    void execute(() => api.deleteBundle(slug), t("bundles.deleted"));
  };
  const deployment = async (slug: string, agent: "codex" | "hermes", undeploy: boolean, dryRun: boolean) => {
    setBusy(true);
    try {
      const result = await api.deployBundle(slug, agent, dryRun, undeploy);
      if (dryRun) setPreview(JSON.stringify(result, null, 2));
      else { setPreview(""); await refresh(); toast.success(t("bundles.changed")); }
    } catch (error) { toast.error(String(error)); }
    finally { setBusy(false); }
  };

  const availableSkills = skills.filter(skill => !bundles.some(bundle => bundle.slug === skill.name));

  return <div className="app-page text-primary">
    <header className="app-page-header flex flex-wrap items-center justify-between gap-3">
      <div>
        <h1 className="app-page-title flex items-center gap-2">{t("bundles.title")}<span className="app-badge">{bundles.length}</span></h1>
        <p className="app-page-subtitle">{t("bundles.subtitle")}</p>
      </div>
      <button className="app-button-primary" disabled={busy} onClick={() => { setDraft(emptyBundle()); setOriginalSlug(undefined); setPreview(""); setSkillQuery({}); }}><Plus className="h-4 w-4" />{t("bundles.new")}</button>
    </header>

    {draft && <section className="app-panel overflow-hidden">
      <div className="border-b border-border-subtle px-5 py-4"><h2 className="text-[15px] font-semibold">{originalSlug ? t("bundles.edit") : t("bundles.new")}</h2></div>
      <div className="space-y-5 p-5">
        <div className="grid gap-4 md:grid-cols-2">
          <label className="text-[13px] text-secondary">{t("bundles.slug")}<input className="app-input mt-1.5 w-full" value={draft.slug} disabled={!!originalSlug} onChange={e => setDraft({ ...draft, slug: e.target.value })} /></label>
          <label className="text-[13px] text-secondary">{t("bundles.description")}<input className="app-input mt-1.5 w-full" value={draft.description} onChange={e => setDraft({ ...draft, description: e.target.value })} /></label>
        </div>
        <label className="block text-[13px] text-secondary">{t("bundles.instructions")}<textarea className="app-input mt-1.5 h-auto w-full py-2.5" rows={3} value={draft.instructions} onChange={e => setDraft({ ...draft, instructions: e.target.value })} /></label>
        <div className="space-y-3">
          <div className="flex items-center justify-between"><h3 className="app-section-title">{t("bundles.stages")}</h3><button className="app-button-secondary" onClick={() => setDraft({ ...draft, stages: [...draft.stages, { id: "", skills: [], after: [], when: null }] })}><Plus className="h-3.5 w-3.5" />{t("bundles.addStage")}</button></div>
          {draft.stages.map((stage, index) => {
            const query = (skillQuery[index] ?? "").trim().toLowerCase();
            const visibleSkills = availableSkills.filter(skill => skill.name.toLowerCase().includes(query)).sort((a, b) => Number(stage.skills.includes(b.id)) - Number(stage.skills.includes(a.id)));
            return <div key={index} className="app-panel-muted space-y-4 p-4">
              <div className="flex items-center gap-3"><span className="app-badge shrink-0">{index + 1}</span><input aria-label={t("bundles.stageId")} placeholder={t("bundles.stageId")} className="app-input min-w-0 flex-1" value={stage.id} onChange={e => patchStage(index, { id: e.target.value })} /><button className="app-button-secondary !px-3" aria-label={t("bundles.removeStage")} disabled={draft.stages.length === 1} onClick={() => setDraft({ ...draft, stages: draft.stages.filter((_, i) => i !== index).map(item => ({ ...item, after: item.after.filter(id => id !== stage.id) })) })}><Trash2 className="h-4 w-4" /></button></div>
              <div>
                <div className="mb-2 flex flex-wrap items-center justify-between gap-2"><span className="app-section-title">Skills · {stage.skills.length}</span><div className="relative w-full sm:w-56"><Search className="absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted" /><input aria-label={t("bundles.searchSkills")} placeholder={t("bundles.searchSkills")} className="app-input w-full pl-9" value={skillQuery[index] ?? ""} onChange={e => setSkillQuery({ ...skillQuery, [index]: e.target.value })} /></div></div>
                <div className="bundle-skill-picker flex max-h-44 flex-wrap content-start gap-2 overflow-y-auto rounded-lg border border-border-subtle bg-surface p-3">{visibleSkills.map(skill => <label key={skill.id} className="flex cursor-pointer items-center gap-1.5 rounded-md border border-border-subtle px-2.5 py-1.5 text-[12px] text-secondary hover:bg-surface-hover"><input type="checkbox" checked={stage.skills.includes(skill.id)} onChange={e => patchStage(index, { skills: e.target.checked ? [...stage.skills, skill.id] : stage.skills.filter(id => id !== skill.id) })} />{skill.name}</label>)}</div>
              </div>
              <div className="grid gap-4 md:grid-cols-2"><fieldset><legend className="mb-1.5 text-[13px] text-secondary">{t("bundles.after")}</legend><div className="flex min-h-10 flex-wrap items-center gap-2">{draft.stages.slice(0, index).map(item => <label key={item.id} className="app-badge cursor-pointer"><input type="checkbox" checked={stage.after.includes(item.id)} onChange={e => patchStage(index, { after: e.target.checked ? [...stage.after, item.id] : stage.after.filter(id => id !== item.id) })} />{item.id}</label>)}</div></fieldset><label className="text-[13px] text-secondary">{t("bundles.when")}<input className="app-input mt-1.5 w-full" value={stage.when ?? ""} onChange={e => patchStage(index, { when: e.target.value || null })} /></label></div>
            </div>;
          })}
        </div>
        <div className="flex justify-end gap-2 border-t border-border-subtle pt-4"><button className="app-button-secondary" onClick={() => setDraft(null)}>{t("bundles.cancel")}</button><button disabled={busy} className="app-button-primary" onClick={save}>{t("bundles.save")}</button></div>
      </div>
    </section>}

    {preview && <pre className="app-panel overflow-auto p-4 text-xs text-secondary">{preview}</pre>}
    {bundles.length === 0 && !draft && <div className="app-panel-muted flex flex-col items-center gap-2 px-6 py-16 text-center"><Layers3 className="h-9 w-9 text-faint" /><h2 className="text-[15px] font-medium">{t("bundles.emptyTitle")}</h2><p className="app-page-subtitle">{t("bundles.emptyDescription")}</p></div>}
    <div className="grid gap-4 xl:grid-cols-2">{bundles.map(bundle => <article key={bundle.slug} className="app-panel flex flex-col overflow-hidden">
      <div className="flex items-start justify-between gap-4 border-b border-border-subtle px-5 py-4"><div className="min-w-0"><h2 className="truncate text-[15px] font-semibold">{bundle.slug}</h2><p className="mt-1 text-[13px] text-muted">{bundle.description}</p></div><div className="flex shrink-0 gap-2"><button className="app-button-secondary" onClick={() => edit(bundle)}>{t("bundles.edit")}</button><button className="app-button-secondary !px-3" aria-label={t("bundles.delete")} onClick={() => remove(bundle.slug)}><Trash2 className="h-4 w-4 text-red-400" /></button></div></div>
      <div className="flex-1 space-y-4 px-5 py-4"><div className="flex flex-wrap gap-2 text-[12px]"><code className="app-badge">Codex: ${bundle.slug}</code><code className="app-badge">Hermes: /{bundle.slug}</code></div><ol className="space-y-2">{bundle.stages.map((stage, index) => <li key={stage.id} className="flex gap-3 rounded-lg border border-border-subtle bg-bg-secondary px-3 py-2.5 text-[12px]"><span className="text-faint">{index + 1}</span><div className="min-w-0"><strong className="font-medium text-secondary">{stage.id}</strong><p className="break-words text-muted">{stage.skills.map(id => skills.find(skill => skill.id === id)?.name ?? id).join(", ")}</p>{stage.after.length > 0 && <p className="text-faint">← {stage.after.join(", ")}</p>}{stage.when && <p className="text-faint">? {stage.when}</p>}</div></li>)}</ol></div>
      <div className="grid gap-3 border-t border-border-subtle px-5 py-4 sm:grid-cols-2">{(["codex", "hermes"] as const).map(agent => <div key={agent} className="rounded-lg border border-border-subtle bg-bg-secondary p-3"><div className="mb-2 flex items-center justify-between gap-2 text-[12px]"><span className="font-medium capitalize text-secondary">{agent}</span><span className={deployed(bundle.slug, agent) ? "text-accent-light" : "text-muted"}>{deployed(bundle.slug, agent) ? t("bundles.deployed") : t("bundles.notDeployed")}</span></div><div className="flex gap-2"><button disabled={busy} className="app-button-secondary flex-1 !py-1.5" onClick={() => void deployment(bundle.slug, agent, false, true)}>{t("bundles.preview")}</button><button disabled={busy} className="app-button-primary flex-1 !py-1.5" onClick={() => void deployment(bundle.slug, agent, deployed(bundle.slug, agent), false)}>{deployed(bundle.slug, agent) ? t("bundles.undeploy") : t("bundles.deploy")}</button></div></div>)}</div>
    </article>)}</div>
  </div>;
}
