import { useEffect, useState } from "react";
import { CaseSensitive, FolderOpen, Plus, Save, Sparkles, Trash2 } from "lucide-react";
import { api, errorMessage } from "../api";
import type { PromptCleanerConfig, PromptCleanerRule } from "../types";

const blankRule = (ruleType: PromptCleanerRule["ruleType"]): PromptCleanerRule => ({
  ruleType,
  find: "",
  replace: "",
  ignoreCase: false,
});

export default function PromptCleanerWorkbench({ compact = false }: { compact?: boolean }) {
  const [config, setConfig] = useState<PromptCleanerConfig>({ root: "", recursive: true, rules: [blankRule("delete")] });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("正在读取规则配置...");
  const [isError, setIsError] = useState(false);

  useEffect(() => {
    void api.promptCleanerLoad()
      .then((value) => { setConfig(value); setMessage("规则按从上到下的顺序执行，空白查找内容会被忽略。"); })
      .catch((error) => { setMessage(errorMessage(error)); setIsError(true); });
  }, []);

  function updateRule(index: number, patch: Partial<PromptCleanerRule>) {
    setConfig((current) => ({
      ...current,
      rules: current.rules.map((rule, ruleIndex) => ruleIndex === index ? { ...rule, ...patch } : rule),
    }));
  }

  function addRule(ruleType: PromptCleanerRule["ruleType"]) {
    setConfig((current) => ({ ...current, rules: [...current.rules, blankRule(ruleType)] }));
  }

  async function save() {
    setBusy(true);
    setIsError(false);
    try {
      await api.promptCleanerSave(config);
      setMessage(`已保存 ${config.rules.length} 条规则。`);
    } catch (error) {
      setMessage(errorMessage(error));
      setIsError(true);
    } finally {
      setBusy(false);
    }
  }

  async function run() {
    setBusy(true);
    setIsError(false);
    setMessage("正在扫描并直接写回 TXT 文件，请勿关闭 Mosaic。");
    try {
      const result = await api.promptCleanerRun(config);
      setMessage(`处理完成：扫描 ${result.scanned} 个 TXT，修改 ${result.changed} 个文件，${result.matchedRules} 次规则生效。`);
    } catch (error) {
      setMessage(errorMessage(error));
      setIsError(true);
    } finally {
      setBusy(false);
    }
  }

  return <section className={"prompt-workbench" + (compact ? " compact" : "")}>
    <div className="prompt-root">
      <FolderOpen size={19} />
      <label><span>TXT 根目录</span><input value={config.root} onChange={(event) => setConfig({ ...config, root: event.target.value })} placeholder="例如 D:\training\prompts" /></label>
      <label className="prompt-check"><input type="checkbox" checked={config.recursive} onChange={(event) => setConfig({ ...config, recursive: event.target.checked })} />包含所有子目录</label>
    </div>

    <div className="prompt-rule-toolbar">
      <div><strong>清洗规则</strong><span>{config.rules.length} 条，按表格顺序执行</span></div>
      <button className="btn small" onClick={() => addRule("delete")}><Plus size={14} /> 添加删除规则</button>
      <button className="btn small" onClick={() => addRule("replace")}><Plus size={14} /> 添加覆盖规则</button>
    </div>

    <div className="prompt-rule-table">
      <div className="prompt-rule-header"><span>类型</span><span>查找内容</span><span>替换为</span><span>匹配</span><span /></div>
      {config.rules.map((rule, index) => <div className="prompt-rule-row" key={index}>
        <span className={`prompt-rule-kind ${rule.ruleType}`}>{rule.ruleType === "delete" ? "删除" : "覆盖"}</span>
        <input value={rule.find} onChange={(event) => updateRule(index, { find: event.target.value })} placeholder="输入要匹配的词语或片段" />
        <input value={rule.replace} disabled={rule.ruleType === "delete"} onChange={(event) => updateRule(index, { replace: event.target.value })} placeholder={rule.ruleType === "delete" ? "删除规则无需填写" : "输入覆盖后的内容"} />
        <label className="prompt-case" title="忽略英文大小写"><input type="checkbox" checked={rule.ignoreCase} onChange={(event) => updateRule(index, { ignoreCase: event.target.checked })} /><CaseSensitive size={18} /></label>
        <button className="prompt-remove" onClick={() => setConfig({ ...config, rules: config.rules.filter((_, ruleIndex) => ruleIndex !== index) })} aria-label="删除此规则"><Trash2 size={16} /></button>
      </div>)}
      {config.rules.length === 0 && <div className="prompt-rule-empty">还没有规则，请从上方添加删除或覆盖规则。</div>}
    </div>

    <footer className="prompt-workbench-foot">
      <p className={isError ? "error" : ""}>{message}</p>
      <button className="btn" disabled={busy} onClick={() => void save()}><Save size={15} /> 保存规则</button>
      <button className="btn primary" disabled={busy} onClick={() => void run()}><Sparkles size={15} /> {busy ? "处理中..." : "覆盖处理全部 TXT"}</button>
    </footer>
  </section>;
}
