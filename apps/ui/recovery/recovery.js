(() => {
  const byId = id => document.getElementById(id);
  let busy = false, loading = false;
  const invoke = action => window.__TAURI__.core.invoke('recovery_action', {action});
  const actions = () => byId('recovery-actions').querySelectorAll('button');
  const reassurance = 'The service and its work can continue independently of this window.';
  const media=matchMedia('(prefers-color-scheme: dark)');
  function theme() {
    let value='terminal';try{value=localStorage.getItem('thesaurus.theme.v1')||value;}catch{}
    if(!['terminal','midnight','phosphor','paper','system'].includes(value))value='terminal';
    document.documentElement.dataset.theme=value==='system'?(media.matches?'terminal':'paper'):value;
  }
  theme();media.addEventListener('change',theme);window.addEventListener('storage',theme);
  function render(state) {
    const otherDesktop = state.phase === 'other_desktop';
    byId('phase').textContent = state.phase.replaceAll('_', ' ');
    byId('heading').textContent = state.headline;
    byId('detail').textContent = otherDesktop
      ? 'The service reports another desktop connection for this workspace. Continue in that copy, or follow these steps to connect here.'
      : state.detail;
    byId('other-desktop-help').hidden = !otherDesktop;
    byId('start').hidden = !state.can_start;
    byId('enable').hidden = !state.can_enable_windows;
  }
  async function refresh() {
    if (busy || loading) return;
    loading = true;
    try {render(await invoke('status'));}
    catch {byId('error').hidden=false;byId('error').textContent='The desktop connection could not be checked. Reopen the app if retry is unavailable.';}
    finally {loading=false;}
  }
  async function run(action) {
    if (busy) return;
    busy = true;byId('error').hidden = true;
    actions().forEach(b=>b.disabled=true);
    byId('hint').textContent='Checking your workspace…';
    try {render(await invoke(action));}
    catch (error) {byId('error').hidden=false;byId('error').textContent=String(error);}
    finally {busy=false;actions().forEach(b=>b.disabled=false);byId('hint').textContent=reassurance;await refresh();}
  }
  if (!window.__TAURI__) {
    byId('heading').textContent='Open the Thesaurus desktop app';
    byId('detail').textContent='Workspace recovery controls are available inside the desktop app.';
    byId('recovery-actions').hidden=true;return;
  }
  for (const [id,action] of [['retry','retry'],['start','start_service'],['enable','enable_windows'],['quit','quit']]) byId(id).addEventListener('click',()=>run(action));
  window.__TAURI__.event.listen('desktop-recovery',event=>render(event.payload)).catch(()=>{});
  refresh();setInterval(()=>{if(!document.hidden)refresh();},1000);
})();
