import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Plus, Trash2 } from "lucide-react";
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

  return <div className="mx-auto max-w-5xl space-y-6 p-6 text-primary">
    <header className="flex items-center justify-between">
      <div><h1 className="text-2xl font-semibold">{t("bundles.title")}</h1><p className="mt-1 text-sm text-muted">{t("bundles.subtitle")}</p></div>
      <button className="rounded-lg bg-emerald-600 px-4 py-2 text-white disabled:opacity-50" disabled={busy} onClick={() => { setDraft(emptyBundle()); setOriginalSlug(undefined); setPreview(""); }}><Plus className="mr-1 inline h-4 w-4" />{t("bundles.new")}</button>
    </header>
    {draft && <section className="space-y-4 rounded-xl border border-border bg-surface p-5">
      <h2 className="text-lg font-medium">{originalSlug ? t("bundles.edit") : t("bundles.new")}</h2>
      <div className="grid gap-3 md:grid-cols-2">
        <label className="text-sm">{t("bundles.slug")}<input className="mt-1 w-full rounded border border-border bg-background p-2" value={draft.slug} disabled={!!originalSlug} onChange={e => setDraft({ ...draft, slug: e.target.value })} /></label>
        <label className="text-sm">{t("bundles.description")}<input className="mt-1 w-full rounded border border-border bg-background p-2" value={draft.description} onChange={e => setDraft({ ...draft, description: e.target.value })} /></label>
      </div>
      <label className="block text-sm">{t("bundles.instructions")}<textarea className="mt-1 w-full rounded border border-border bg-background p-2" rows={3} value={draft.instructions} onChange={e => setDraft({ ...draft, instructions: e.target.value })} /></label>
      <div className="space-y-3"><h3 className="font-medium">{t("bundles.stages")}</h3>
        {draft.stages.map((stage, index) => <div key={index} className="rounded-lg border border-border p-3">
          <div className="flex gap-2"><input aria-label={t("bundles.stageId")} placeholder={t("bundles.stageId")} className="w-full rounded border border-border bg-background p-2" value={stage.id} onChange={e => patchStage(index, { id: e.target.value })} /><button aria-label={t("bundles.removeStage")} disabled={draft.stages.length === 1} onClick={() => setDraft({ ...draft, stages: draft.stages.filter((_, i) => i !== index) })}><Trash2 className="h-4 w-4" /></button></div>
          <div className="mt-2 flex flex-wrap gap-2">{skills.filter(skill => !bundles.some(bundle => bundle.slug === skill.name)).map(skill => <label key={skill.id} className="rounded border border-border px-2 py-1 text-xs"><input type="checkbox" className="mr-1" checked={stage.skills.includes(skill.id)} onChange={e => patchStage(index, { skills: e.target.checked ? [...stage.skills, skill.id] : stage.skills.filter(id => id !== skill.id) })} />{skill.name}</label>)}</div>
          <div className="mt-3 grid gap-2 md:grid-cols-2"><label className="text-xs">{t("bundles.after")}<select multiple className="mt-1 w-full rounded border border-border bg-background p-2" value={stage.after} onChange={e => patchStage(index, { after: Array.from(e.target.selectedOptions, option => option.value) })}>{draft.stages.filter((_, i) => i !== index).map(item => <option key={item.id} value={item.id}>{item.id}</option>)}</select></label><label className="text-xs">{t("bundles.when")}<input className="mt-1 w-full rounded border border-border bg-background p-2" value={stage.when ?? ""} onChange={e => patchStage(index, { when: e.target.value || null })} /></label></div>
        </div>)}
        <button className="text-sm text-emerald-500" onClick={() => setDraft({ ...draft, stages: [...draft.stages, { id: "", skills: [], after: [], when: null }] })}>+ {t("bundles.addStage")}</button>
      </div>
      <div className="flex gap-2"><button disabled={busy} className="rounded bg-emerald-600 px-4 py-2 text-white disabled:opacity-50" onClick={save}>{t("bundles.save")}</button><button className="rounded border border-border px-4 py-2" onClick={() => setDraft(null)}>{t("bundles.cancel")}</button></div>
    </section>}
    {preview && <pre className="overflow-auto rounded-lg border border-border bg-surface p-4 text-xs">{preview}</pre>}
    <div className="space-y-3">{bundles.map(bundle => <article key={bundle.slug} className="rounded-xl border border-border bg-surface p-5">
      <div className="flex items-start justify-between gap-4"><div><h2 className="text-lg font-medium">{bundle.slug}</h2><p className="text-sm text-muted">{bundle.description}</p><p className="mt-1 font-mono text-xs text-muted">Codex: ${bundle.slug} · Hermes: /{bundle.slug}</p></div><div className="flex gap-2"><button className="text-sm text-emerald-500" onClick={() => edit(bundle)}>{t("bundles.edit")}</button><button aria-label={t("bundles.delete")} onClick={() => remove(bundle.slug)}><Trash2 className="h-4 w-4 text-red-400" /></button></div></div>
      <ol className="my-4 space-y-2">{bundle.stages.map(stage => <li key={stage.id} className="rounded border border-border p-2 text-sm"><strong>{stage.id}</strong> · {stage.skills.map(id => skills.find(skill => skill.id === id)?.name ?? id).join(", ")}{stage.after.length > 0 && <span className="ml-2 text-muted">← {stage.after.join(", ")}</span>}{stage.when && <span className="ml-2 text-muted">? {stage.when}</span>}</li>)}</ol>
      <div className="flex flex-wrap gap-2">{(["codex", "hermes"] as const).map(agent => <div key={agent} className="flex items-center gap-2 rounded border border-border px-2 py-1 text-sm"><span>{agent}: {deployed(bundle.slug, agent) ? t("bundles.deployed") : t("bundles.notDeployed")}</span><button disabled={busy} className="text-emerald-500" onClick={() => void deployment(bundle.slug, agent, false, true)}>{t("bundles.preview")}</button><button disabled={busy} className="text-emerald-500" onClick={() => void deployment(bundle.slug, agent, deployed(bundle.slug, agent), false)}>{deployed(bundle.slug, agent) ? t("bundles.undeploy") : t("bundles.deploy")}</button></div>)}</div>
    </article>)}</div>
  </div>;
}
