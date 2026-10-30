//! Focus management for workspace dialogs.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
let stopWorkspaceDialogs = null;

export function initializeWorkspaceDialogs(enabled) {
  if (!enabled) {
    stopWorkspaceDialogs?.();
    stopWorkspaceDialogs = null;
    return;
  }
  if (stopWorkspaceDialogs) return;

  // Native <dialog> elements (including the mobile navigation drawer) own
  // their own lifecycle. This manager upgrades only the existing overlays.
  const selector = '.modal-overlay [role="dialog"][aria-modal="true"]';
  const controlSelector = 'a[href],button,input:not([type="hidden"]),select,textarea,summary,[contenteditable="true"],[tabindex]';
  const records = new Map();
  const madeInert = new Set();
  let active = null;
  let scheduled = false;
  let disposed = false;
  let redirecting = false;
  let invoker = null;

  const rendered = element => element instanceof HTMLElement && element.isConnected
    && !element.closest('[hidden]') && element.getClientRects().length > 0
    && getComputedStyle(element).visibility !== 'hidden';
  const available = element => rendered(element) && !element.matches(':disabled')
    && !element.closest('[inert]');
  const focus = element => {
    if (!available(element)) return false;
    element.focus({preventScroll: true});
    return document.activeElement === element;
  };
  const controls = dialog => {
    const nodes = [...dialog.querySelectorAll(controlSelector)].filter(element =>
      available(element) && (element.tabIndex >= 0 || (element.isContentEditable && !element.hasAttribute('tabindex'))));
    // A radio group is one Tab stop, matching native sequential navigation.
    return nodes.filter(element => {
      if (!(element instanceof HTMLInputElement) || element.type !== 'radio' || !element.name) return true;
      const group = nodes.filter(other => other instanceof HTMLInputElement && other.type === 'radio'
        && other.name === element.name && other.form === element.form);
      return element === (group.find(other => other.checked) || group[0]);
    }).sort((a, b) => {
      const aOrder = a.tabIndex > 0 ? a.tabIndex : Infinity;
      const bOrder = b.tabIndex > 0 ? b.tabIndex : Infinity;
      return aOrder - bOrder;
    });
  };
  const restoreInert = () => {
    for (const element of madeInert) element.removeAttribute('inert');
    madeInert.clear();
  };
  const inertOutside = dialog => {
    for (let branch = dialog; branch.parentElement; branch = branch.parentElement) {
      for (const sibling of branch.parentElement.children) {
        if (sibling === branch || sibling.hasAttribute('inert') || sibling.matches('script,style,link,meta')) continue;
        sibling.setAttribute('inert', '');
        madeInert.add(sibling);
      }
      if (branch.parentElement === document.body) break;
    }
  };
  const focusInside = dialog => {
    if (dialog.contains(document.activeElement) && available(document.activeElement)) return;
    const preferred = dialog.querySelector('[autofocus], [data-dialog-initial-focus]')
      || dialog.querySelector('.document-line.cited')
      || dialog.querySelector('input:not([type="hidden"]):not(:disabled), textarea:not(:disabled), select:not(:disabled)');
    if (focus(preferred)) return;
    if (focus(controls(dialog)[0])) return;
    focus(dialog);
  };
  const restoreFocus = opener => {
    if (focus(opener)) return;
    focus([...document.querySelectorAll('.workspace-nav [aria-current="page"], .mobile-menu-trigger')].find(available));
  };
  const originalOpener = record => {
    let opener = record?.opener;
    const visited = new Set();
    while (opener && !available(opener)) {
      const owner = opener.closest?.(selector);
      if (!owner || visited.has(owner)) break;
      visited.add(owner);
      opener = records.get(owner)?.opener;
    }
    return opener;
  };

  function sync() {
    scheduled = false;
    if (disposed) return;
    // Restore our own inert attributes before discovering a newly exposed
    // lower dialog. Pre-existing inert attributes are never removed.
    restoreInert();
    const dialogs = [...document.querySelectorAll(selector)].filter(rendered);
    const previous = active;
    const previousRecord = records.get(previous);
    const next = dialogs.at(-1) || null;
    const returnTo = originalOpener(previousRecord);

    for (const dialog of dialogs) {
      if (records.has(dialog)) continue;
      const replacing = previous && !dialogs.includes(previous);
      const focused = document.activeElement;
      const opener = replacing ? returnTo : (available(invoker) && !dialog.contains(invoker)
        ? invoker : (!dialog.contains(focused) ? focused : null));
      const originalTabindex = dialog.getAttribute('tabindex');
      records.set(dialog, {opener, originalTabindex});
      if (originalTabindex === null) dialog.setAttribute('tabindex', '-1');
    }
    for (const [dialog, record] of records) {
      if (dialogs.includes(dialog)) continue;
      if (record.originalTabindex === null) dialog.removeAttribute('tabindex');
      else dialog.setAttribute('tabindex', record.originalTabindex);
      records.delete(dialog);
    }
    active = next;
    if (active) {
      inertOutside(active);
      if (previous !== active && active.contains(returnTo)) focus(returnTo);
      focusInside(active);
    } else if (previous) {
      invoker = null;
      restoreFocus(returnTo);
    }
  }
  const schedule = () => {
    if (scheduled || disposed) return;
    scheduled = true;
    // Focus containment and inertness are semantic state, not animation.
    // Safari can suspend animation frames in a background window even while
    // its DOM and accessibility actions continue to update.
    queueMicrotask(sync);
  };
  const rememberPointer = event => {
    const target = event.target instanceof Element ? event.target.closest(controlSelector) : null;
    if (target) invoker = target;
  };
  const keydown = event => {
    if (scheduled) sync();
    if (!active) {
      if (event.key !== 'Tab') invoker = document.activeElement;
      return;
    }
    // The app's global Settings shortcut otherwise changes the underlying
    // route before its existing modal checks. Keep the protected task in place.
    if (event.metaKey && event.key === ',') {
      event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }
    // Escape continues to the existing close handlers, preserving their state
    // and theme-preview cancellation semantics.
    if (event.key !== 'Tab' || event.altKey || event.ctrlKey || event.metaKey) return;
    const nodes = controls(active);
    const current = document.activeElement;
    const index = nodes.indexOf(current);
    let target;
    if (index >= 0) {
      target = nodes[(index + (event.shiftKey ? -1 : 1) + nodes.length) % nodes.length];
    } else if (active.contains(current)) {
      // Cited source lines intentionally have tabindex=-1. Continue from the
      // focused line in document order instead of jumping to the close button.
      target = event.shiftKey
        ? nodes.filter(node => current.compareDocumentPosition(node) & Node.DOCUMENT_POSITION_PRECEDING).at(-1)
        : nodes.find(node => current.compareDocumentPosition(node) & Node.DOCUMENT_POSITION_FOLLOWING);
    }
    target ||= event.shiftKey ? nodes.at(-1) : nodes[0];
    event.preventDefault();
    // Handle Tab once. Theme's older local trap must not move focus a second time.
    event.stopImmediatePropagation();
    if (!focus(target)) focus(active);
  };
  const focusin = event => {
    if (!active || active.contains(event.target) || redirecting) return;
    redirecting = true;
    focusInside(active);
    redirecting = false;
  };
  const observer = new MutationObserver(schedule);
  observer.observe(document.body, {
    childList: true, subtree: true, attributes: true,
    // Inert is deliberately excluded: our own writes must not cause a loop.
    attributeFilter: ['hidden', 'style', 'class', 'open', 'disabled', 'role', 'aria-modal', 'tabindex']
  });
  document.addEventListener('pointerdown', rememberPointer, true);
  // Assistive technology may activate a control with click and no pointerdown.
  document.addEventListener('click', rememberPointer, true);
  document.addEventListener('keydown', keydown, true);
  document.addEventListener('focusin', focusin, true);
  stopWorkspaceDialogs = () => {
    disposed = true;
    scheduled = false;
    observer.disconnect();
    document.removeEventListener('pointerdown', rememberPointer, true);
    document.removeEventListener('click', rememberPointer, true);
    document.removeEventListener('keydown', keydown, true);
    document.removeEventListener('focusin', focusin, true);
    restoreInert();
    for (const [dialog, record] of records) {
      if (record.originalTabindex === null) dialog.removeAttribute('tabindex');
      else dialog.setAttribute('tabindex', record.originalTabindex);
    }
    records.clear();
    active = null;
  };
  schedule();
}
"#)]
extern "C" {
    /// Passing `false` fully detaches
    /// the manager and restores any inert/tabindex attributes it owned.
    #[wasm_bindgen(js_name = initializeWorkspaceDialogs)]
    pub(super) fn initialize(enabled: bool);
}
