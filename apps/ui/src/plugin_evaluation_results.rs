use super::plugin_runs::Citations;
use super::*;

#[component]
pub(super) fn PluginEvaluationResults(
    receipts: Vec<PluginEvaluationReceipt>,
    notice: RwSignal<String>,
) -> impl IntoView {
    view! {<div class="plugin-evaluation-results">{(!receipts.is_empty()).then(||view!{<h3>"Plugin evaluation checks"</h3><p class="muted">"Separate reported checks on pinned eval inputs. Pass/fail uses the saved case threshold. These checks do not replace the original scores or baselines."</p>})}
        {receipts.into_iter().map(|r|view!{<article class="plugin-delivery"><h4>{format!("{} · {}",r.plugin_id,r.contribution_id)}</h4><small class="muted">{format!("Evaluation {} · package {} · configuration v{}",r.evaluation_run_id,r.package_version,r.configuration_version)}</small>
            {r.results.iter().cloned().map(|result|view!{<section><h4>{format!("{} · {}",if result.passed{"PASS"}else{"FAIL"},result.check.title)}</h4><p class="muted">{format!("Case {} · reported score {:.3} · saved threshold {:.3}",result.check.case_id,result.check.score,result.threshold)}</p><p class="workflow-body">{result.check.detail}</p><Citations evidence=result.check.evidence notice=notice/></section>}).collect_view()}
            <details><summary>"Check receipt and provenance"</summary><pre>{serde_json::to_string_pretty(&r).unwrap_or_default()}</pre></details>
        </article>}).collect_view()}
    </div>}
}
