// Dependency-free regression checks for the workspace dialog focus manager.
// This small DOM model checks lifecycle and keyboard routing with no animation
// frames delivered, reproducing background-window scheduling. It does not
// replace live browser, VoiceOver, layout, or accessibility-tree verification.
// Run from any directory: node /path/to/thesaurus/scripts/test-dialogs.mjs

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
const MODAL = '.modal-overlay [role="dialog"][aria-modal="true"]';
let document;
class Element {
  constructor(tag, attributes={}) {this.tag=tag;this.attrs=new Map(Object.entries(attributes));this.children=[];this.parentElement=null;this.style={};this.checked=false;this.form=null;}
  append(...nodes){for(const n of nodes){n.parentElement=this;this.children.push(n);}}
  remove(){const p=this.parentElement;if(p)p.children=p.children.filter(n=>n!==this);this.parentElement=null;}
  get isConnected(){return this===document.body || Boolean(this.parentElement?.isConnected);}
  get tabIndex(){return this.hasAttribute('tabindex')?Number(this.getAttribute('tabindex')):(['button','input','textarea','select','summary'].includes(this.tag)?0:-1);}
  get type(){return this.getAttribute('type')||'text';}
  get name(){return this.getAttribute('name')||'';}
  get isContentEditable(){return this.getAttribute('contenteditable')==='true';}
  getAttribute(k){return this.attrs.get(k)??null;}
  hasAttribute(k){return this.attrs.has(k);}
  setAttribute(k,v){this.attrs.set(k,String(v));}
  removeAttribute(k){this.attrs.delete(k);}
  contains(n){return n===this || this.children.some(c=>c.contains(n));}
  getClientRects(){return this.isConnected&&!this.closest('[hidden]')?[{}]:[];}
  closest(s){for(let n=this;n;n=n.parentElement)if(n.matches(s))return n;return null;}
  matches(s){
    if(s.includes(','))return s.split(',').some(part=>this.matches(part));
    if(s===MODAL)return this.getAttribute('role')==='dialog'&&this.getAttribute('aria-modal')==='true'&&Boolean(this.parentElement?.closest('.modal-overlay'));
    if(s===':disabled')return this.hasAttribute('disabled')||Boolean(this.parentElement?.closest('fieldset[disabled]'));
    if(s==='fieldset[disabled]')return this.tag==='fieldset'&&this.hasAttribute('disabled');
    if(s==='.workspace-nav [aria-current="page"]')return this.getAttribute('aria-current')==='page'&&Boolean(this.parentElement?.closest('.workspace-nav'));
    if(s.startsWith('.'))return s.slice(1).split('.').every(c=>(this.getAttribute('class')||'').split(' ').includes(c));
    if(s==='[tabindex]')return this.hasAttribute('tabindex');
    if(s==='[contenteditable="true"]')return this.isContentEditable;
    if(/^\[[a-z-]+\]$/.test(s))return this.hasAttribute(s.slice(1,-1));
    if(s.startsWith('input'))return this.tag==='input'&&(!s.includes('[type="hidden"]')||this.type!=='hidden')&&(!s.includes(':disabled')||!this.matches(':disabled'));
    if(s.startsWith('textarea')||s.startsWith('select'))return this.tag===s.split(':')[0]&&(!s.includes(':disabled')||!this.matches(':disabled'));
    if(s==='a[href]')return this.tag==='a'&&this.hasAttribute('href');
    return this.tag===s;
  }
  querySelectorAll(s){return this.children.flatMap(n=>[...(n.matches(s)?[n]:[]),...n.querySelectorAll(s)]);}
  querySelector(s){return this.querySelectorAll(s)[0]||null;}
  focus(){if(!this.isConnected||this.closest('[inert]')||this.matches(':disabled'))return;document.activeElement=this;document.dispatch('focusin',{target:this});}
  compareDocumentPosition(other){const all=[document.body,...document.body.querySelectorAll('*')];return all.indexOf(this)<all.indexOf(other)?4:2;}
}
// Match all descendants for relative DOM-order checks.
const match=Element.prototype.matches;Element.prototype.matches=function(s){return s==='*'||match.call(this,s);};
class Input extends Element {constructor(attrs={}){super('input',attrs);}}
const listeners=new Map(),observers=new Set(),frames=new Map();let serial=0;
document={body:new Element('body'),activeElement:null,querySelector:s=>document.body.querySelector(s),querySelectorAll:s=>document.body.querySelectorAll(s),addEventListener(t,f){if(!listeners.has(t))listeners.set(t,new Set());listeners.get(t).add(f);},removeEventListener(t,f){listeners.get(t)?.delete(f);},dispatch(t,e){for(const f of listeners.get(t)||[])f(e);}};
document.activeElement=document.body;
Object.assign(globalThis,{document,Element,HTMLElement:Element,HTMLInputElement:Input,Node:{DOCUMENT_POSITION_PRECEDING:2,DOCUMENT_POSITION_FOLLOWING:4},getComputedStyle:()=>({visibility:'visible'}),requestAnimationFrame:f=>{frames.set(++serial,f);return serial;},cancelAnimationFrame:id=>frames.delete(id),MutationObserver:class{constructor(callback){this.callback=callback;}observe(){observers.add(this);}disconnect(){observers.delete(this);}}});
const flush=async()=>{await Promise.resolve();await Promise.resolve();assert.equal(frames.size,0,'Dialog semantics must not wait for animation frames');};
const change=async()=>{for(const o of observers)o.callback();await flush();};
const key=(key,props={})=>{const e={key,shiftKey:false,metaKey:false,ctrlKey:false,altKey:false,prevented:false,stopped:false,preventDefault(){this.prevented=true;},stopImmediatePropagation(){this.stopped=true;},...props};document.dispatch('keydown',e);return e;};
const source=await readFile(new URL('../apps/ui/src/dialog_manager.rs', import.meta.url),'utf8');
const js=source.split('inline_js = r#"')[1].split('"#)]')[0];
const {initializeWorkspaceDialogs:init}=await import('data:text/javascript,'+encodeURIComponent(js));
const root=new Element('main');const opener=new Element('button');const existingInert=new Element('div',{inert:'original'});document.body.append(root,existingInert);root.append(opener);opener.focus();
function overlay(){const overlay=new Element('div',{class:'modal-overlay'}),dialog=new Element('section',{role:'dialog','aria-modal':'true'}),close=new Element('button'),field=new Input(),save=new Element('button');dialog.append(close,field,save);overlay.append(dialog);document.body.append(overlay);return{overlay,dialog,close,field,save};}
init(false);assert.equal(observers.size,0);init(true);init(true);assert.equal(observers.size,1);await flush();
// Screen-reader style activation supplies click without a pointer event.
document.dispatch('click',{target:opener});const one=overlay();await change();
assert.equal(document.activeElement,one.field);assert(root.hasAttribute('inert'));assert.equal(existingInert.getAttribute('inert'),'original');
one.save.focus();let e=key('Tab');assert(e.prevented&&e.stopped);assert.equal(document.activeElement,one.close);
e=key('Tab',{shiftKey:true});assert.equal(document.activeElement,one.save);
// Newly disabled controls immediately drop out of the tab sequence.
one.save.setAttribute('disabled','');one.field.focus();key('Tab');assert.equal(document.activeElement,one.close);
e=key(', ',{metaKey:true});assert(!e.prevented);e=key(',',{metaKey:true});assert(e.prevented&&e.stopped);
e=key('Escape');assert(!e.prevented&&!e.stopped);
// Programmatic focus attempts outside the active dialog are redirected.
root.removeAttribute('inert');opener.focus();assert(one.dialog.contains(document.activeElement));
// A renderer replacing the dialog preserves the original invoking control.
one.overlay.remove();const two=overlay();await change();assert.equal(document.activeElement,two.field);two.overlay.remove();await change();assert.equal(document.activeElement,opener);assert(!root.hasAttribute('inert'));assert.equal(existingInert.getAttribute('inert'),'original');
// Cited lines can be the initial focus without becoming an extra Tab stop.
const cited=overlay();const line=new Element('div',{class:'document-line cited',tabindex:'-1'});cited.dialog.children.splice(1,0,line);line.parentElement=cited.dialog;await change();assert.equal(document.activeElement,line);key('Tab');assert.equal(document.activeElement,cited.field);line.focus();key('Tab',{shiftKey:true});assert.equal(document.activeElement,cited.close);
// Nested overlay closure returns to its own invoking control in the lower modal.
cited.field.focus();document.dispatch('click',{target:cited.save});const nested=overlay();await change();assert.equal(document.activeElement,nested.field);nested.overlay.remove();await change();assert.equal(document.activeElement,cited.save);
// A busy dialog with no enabled controls still contains keyboard focus.
for(const n of [cited.close,cited.field,cited.save])n.setAttribute('disabled','');line.remove();await change();assert.equal(document.activeElement,cited.dialog);key('Tab');assert.equal(document.activeElement,cited.dialog);cited.overlay.remove();await change();assert.equal(document.activeElement,opener);
// Native drawers are intentionally not managed by this module.
const native=new Element('dialog',{open:''});root.append(native);await change();assert(!root.hasAttribute('inert'));assert(!native.hasAttribute('tabindex'));
// Enabling/disabling while a dialog exists releases only owned attributes.
const three=overlay();await change();assert(root.hasAttribute('inert'));init(false);assert(!root.hasAttribute('inert'));assert(!three.dialog.hasAttribute('tabindex'));assert.equal(existingInert.getAttribute('inert'),'original');assert.equal(observers.size,0);assert.equal([...listeners.values()].reduce((n,set)=>n+set.size,0),0);assert.equal(frames.size,0);
console.log('Dialog lifecycle checks passed: idempotence, initial focus, wrap, dynamic disabled state, shortcut guard, Escape delegation, focus containment, replacement/opener restoration, native exclusion and complete cleanup.');
