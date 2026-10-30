use super::*;

// A native modal dialog provides focus containment and background inertness.
#[wasm_bindgen(inline_js = r#"
let returnFocus = null;
let widthQuery = null;
let widthChanged = null;
const drawer = () => document.getElementById('mobile-navigation-dialog');

export function showMobileNavigation() {
  const node = drawer();
  if (!node || node.open || !matchMedia('(max-width: 600px)').matches) return !!node?.open;
  returnFocus = document.activeElement;
  node.showModal();
  (node.querySelector('[aria-current="page"]') || node.querySelector('button'))?.focus();
  return true;
}

export function closeMobileNavigation(restore = true) {
  if (!restore) returnFocus = null;
  const node = drawer();
  if (node?.open) node.close();
}

export function restoreMobileNavigationFocus() {
  const target = returnFocus;
  returnFocus = null;
  if (!target) return;
  requestAnimationFrame(() => {
    // An action may have opened another dialog after this drawer closed.
    if (document.querySelector('dialog[open], .modal-overlay [role="dialog"]')) return;
    if (target.isConnected && target.getClientRects().length && !target.closest('[inert]')) {
      target.focus({preventScroll: true});
    } else {
      const heading = document.querySelector('.page-heading h1');
      if (heading?.getClientRects().length) {
        heading.setAttribute('tabindex', '-1');
        heading.focus({preventScroll: true});
      }
    }
  });
}

export function dismissMobileNavigationBackdrop(event) {
  const node = drawer();
  if (!node || event.target !== node) return;
  const rect = node.getBoundingClientRect();
  if (event.clientX < rect.left || event.clientX > rect.right ||
      event.clientY < rect.top || event.clientY > rect.bottom) closeMobileNavigation();
}

export function watchMobileNavigationWidth() {
  stopMobileNavigationWidth();
  widthQuery = matchMedia('(max-width: 600px)');
  widthChanged = () => { if (!widthQuery.matches) closeMobileNavigation(); };
  widthQuery.addEventListener('change', widthChanged);
}

export function stopMobileNavigationWidth() {
  if (widthQuery && widthChanged) widthQuery.removeEventListener('change', widthChanged);
  widthQuery = null;
  widthChanged = null;
  closeMobileNavigation();
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = showMobileNavigation)]
    fn show_navigation() -> bool;
    #[wasm_bindgen(js_name = closeMobileNavigation)]
    fn close_navigation(restore: bool);
    #[wasm_bindgen(js_name = restoreMobileNavigationFocus)]
    fn restore_navigation_focus();
    #[wasm_bindgen(js_name = dismissMobileNavigationBackdrop)]
    fn dismiss_backdrop(event: &JsValue);
    #[wasm_bindgen(js_name = watchMobileNavigationWidth)]
    fn watch_width();
    #[wasm_bindgen(js_name = stopMobileNavigationWidth)]
    fn stop_width();
}

#[component]
pub(super) fn MobileNavigation(
    view: RwSignal<String>,
    hits: RwSignal<Option<Vec<SearchHit>>>,
    open_capture: Callback<()>,
    open_theme: Callback<()>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    watch_width();
    on_cleanup(stop_width);
    Effect::new(move |_| {
        let _ = view.get();
        // Selecting a destination or opening setup closes
        // the drawer. Opening the drawer itself must not retrigger this effect.
        if open.get_untracked() {
            close_navigation(true);
        }
    });

    view! {
        <div class="mobile-navigation">
            <button class="button mobile-menu-trigger" type="button"
                aria-label="Open workspace menu" aria-haspopup="dialog"
                aria-controls="mobile-navigation-dialog" aria-expanded=move || open.get()
                on:click=move |_| open.set(show_navigation())>"Menu"</button>
            <span class="mobile-location">{move || workspace::title(&view.get())}</span>
            <button class="button primary" type="button" on:click=move |_| open_capture.run(())>"Capture"</button>
        </div>
        <dialog id="mobile-navigation-dialog" class="mobile-nav-drawer"
            aria-labelledby="mobile-navigation-title"
            on:close=move |_| { open.set(false); restore_navigation_focus(); }
            on:click=move |ev| dismiss_backdrop(&ev.into())>
            <div class="mobile-nav-heading">
                <h2 id="mobile-navigation-title">"Workspace"</h2>
                <button class="button text" type="button" on:click=move |_| close_navigation(true)>"Close"</button>
            </div>
            <workspace::Navigation view=view hits=hits/>
            <div class="mobile-nav-settings">
                <button class="button secondary" type="button" on:click=move |_| {
                    close_navigation(false); view.set("settings".into()); hits.set(None);
                }>"Settings"</button>
                <button class="button text" type="button" on:click=move |_| {
                    close_navigation(false); open_theme.run(());
                }>"Appearance"</button>
            </div>
        </dialog>
    }
}
