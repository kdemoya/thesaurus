use leptos::prelude::*;
use wasm_bindgen::prelude::*;

// The UI has one implementation for both the local browser and Tauri webview.
// Keep preference handling independent of workspace data and model connections.
#[wasm_bindgen(inline_js = "
const key = 'thesaurus.theme.v1';
const choices = new Set(['terminal','midnight','phosphor','paper','system']);
let preference = 'terminal';
let shown = 'terminal';
let previousFocus = null;
let media;
function valid(value){ return choices.has(value) ? value : 'terminal'; }
function render(value){
  shown = valid(value);
  const resolved = shown === 'system' ? (media.matches ? 'terminal' : 'paper') : shown;
  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.themePreference = shown;
}
export function initializeThemes(){
  media = window.matchMedia('(prefers-color-scheme: dark)');
  try { preference = valid(localStorage.getItem(key)); } catch (_) {}
  render(preference);
  media.addEventListener('change', () => { if(shown === 'system') render(shown); });
  window.addEventListener('storage', event => {
    if(event.key === key){ preference = valid(event.newValue); if(!document.querySelector('.theme-modal')) render(preference); }
  });
}
export function themePreference(){ return preference; }
export function previewTheme(value){ render(value); }
export function saveTheme(value){
  value = valid(value);
  try { localStorage.setItem(key,value); }
  catch (_) { return 'This view cannot save preferences. Allow local storage, or cancel the preview.'; }
  preference = value; render(value); return '';
}
export function focusTheme(){
  previousFocus = document.activeElement;
  requestAnimationFrame(() => document.getElementById('theme-choice')?.focus());
}
export function closeTheme(){
  render(preference);
  previousFocus?.focus(); previousFocus = null;
}
export function trapThemeTab(event){
  if(event.key !== 'Tab') return;
  const nodes = [...document.querySelector('.theme-modal').querySelectorAll('button:not([disabled]),select')];
  const first = nodes[0], last = nodes[nodes.length-1];
  if(event.shiftKey && document.activeElement === first){event.preventDefault();last?.focus();}
  else if(!event.shiftKey && document.activeElement === last){event.preventDefault();first?.focus();}
}
")]
extern "C" {
    #[wasm_bindgen(js_name = initializeThemes)]
    pub fn initialize();
    #[wasm_bindgen(js_name = themePreference)]
    fn preference() -> String;
    #[wasm_bindgen(js_name = previewTheme)]
    fn preview(value: &str);
    #[wasm_bindgen(js_name = saveTheme)]
    fn save(value: &str) -> String;
    #[wasm_bindgen(js_name = focusTheme)]
    fn focus();
    #[wasm_bindgen(js_name = closeTheme)]
    fn close();
    #[wasm_bindgen(js_name = trapThemeTab)]
    fn trap_tab(event: &web_sys::KeyboardEvent);
}

#[component]
pub fn ThemeSettings(open: RwSignal<bool>) -> impl IntoView {
    let choice = RwSignal::new(preference());
    let error = RwSignal::new(String::new());
    let was_open = RwSignal::new(false);
    Effect::new(move |_| {
        if open.get() {
            choice.set(preference());
            error.set(String::new());
            was_open.set(true);
            focus();
        } else if was_open.get_untracked() {
            close();
            was_open.set(false);
        }
    });
    view! {
        <Show when=move||open.get()>
            <div class="modal-overlay">
                <section class="modal theme-modal" role="dialog" aria-modal="true" aria-labelledby="theme-title" on:keydown=move|ev|trap_tab(&ev)>
                    <div class="modal-heading"><h2 id="theme-title">"Appearance"</h2><button class="close-button" aria-label="Cancel theme preview" on:click=move|_|open.set(false)>"×"</button></div>
                    <label for="theme-choice">"Theme"</label>
                    <select id="theme-choice" prop:value=move||choice.get() on:change=move|ev|{let value=event_target_value(&ev);preview(&value);choice.set(value);error.set(String::new());}>
                        <option value="terminal">"Terminal · original dark"</option>
                        <option value="midnight">"Midnight · blue dark"</option>
                        <option value="phosphor">"Phosphor · green dark"</option>
                        <option value="paper">"Paper · warm light"</option>
                        <option value="system">"Follow system · Terminal / Paper"</option>
                    </select>
                    <p class="muted">"Preview changes throughout the interface. Apply saves this choice on this device; Cancel or Escape restores your saved theme."</p>
                    <section class="theme-sample" aria-label="Theme preview">
                        <div class="theme-sample-heading">"Work / preview"</div>
                        <div class="theme-sample-row"><span class="orange">"? review"</span><span>"Confirm the delivery plan"</span></div>
                        <div class="theme-sample-row selected"><span class="violet">"+ accepted"</span><span>"Prepare project brief"</span></div>
                        <div class="theme-sample-row"><span class="theme-success">"✓ complete"</span><span class="muted">"Source indexed"</span></div>
                        <p><span class="source-link">"meeting:12–15"</span>" · "<span class="muted">"exact evidence"</span></p>
                    </section>
                    <Show when=move||!error.get().is_empty()><p class="theme-error" role="alert">{move||error.get()}</p></Show>
                    <div class="modal-footer"><button class="button text" on:click=move|_|open.set(false)>"Cancel"</button><button class="button primary" on:click=move|_|{let message=save(&choice.get_untracked());if message.is_empty(){open.set(false);}else{error.set(message);}}>"Apply theme"</button></div>
                </section>
            </div>
        </Show>
    }
}
