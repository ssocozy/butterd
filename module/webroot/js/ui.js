import {icon} from './icons.js';
export const $ = id => document.getElementById(id);
let noticeTimer;
export function notify(message, error = false) {
  clearTimeout(noticeTimer); const host = $('feedback'); host.textContent = message; host.dataset.error = String(error); host.hidden = !message;
  if (message && !error) noticeTimer = setTimeout(() => { host.hidden = true; }, 5000);
}
export function button(text, action, enabled = true, style = 'secondary') {
  const node = document.createElement('button'); node.type = 'button'; node.className = style; node.textContent = text; node.dataset.capable = String(enabled); node.disabled = !enabled; node.addEventListener('click', action); return node;
}
export function paragraph(text, className = 'caption') { const node = document.createElement('p'); node.className = className; node.textContent = text; return node; }
export function actions(...buttons) { const node = document.createElement('div'); node.className = 'quick-actions'; node.append(...buttons); return node; }
export function setBusy(busy) { document.body.setAttribute('aria-busy', String(busy)); document.querySelectorAll('button[data-capable],input[data-capable]').forEach(node => { node.disabled = busy || node.dataset.capable === 'false'; }); }
const selectors = new Map();
let choiceDialog;
export function updateSelectors() {
  if (!choiceDialog) {
    choiceDialog = document.createElement('dialog'); choiceDialog.id = 'choice-dialog'; choiceDialog.setAttribute('aria-labelledby', 'choice-title');
    const heading = document.createElement('div'); heading.className = 'sheet-heading';
    const title = document.createElement('h2'); title.id = 'choice-title'; const close = button('', () => choiceDialog.close(), true, 'icon-button'); close.append(icon('x')); close.setAttribute('aria-label', 'Close selection'); heading.append(title, close);
    const list = document.createElement('div'); list.id = 'choice-list'; list.setAttribute('role', 'listbox'); choiceDialog.append(heading,list); document.body.append(choiceDialog);
    list.addEventListener('keydown', event => {
      if (!['ArrowUp','ArrowDown','Home','End'].includes(event.key)) return;
      const options = [...list.querySelectorAll('button:not(:disabled)')]; const current = options.indexOf(document.activeElement);
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? options.length-1 : (current + (event.key === 'ArrowDown' ? 1 : -1) + options.length) % options.length;
      options[next]?.focus(); event.preventDefault();
    });
    choiceDialog.addEventListener('click', event => { if (event.target === choiceDialog) { const bounds = choiceDialog.getBoundingClientRect(); if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) choiceDialog.close(); } });
  }
  for (const select of document.querySelectorAll('select')) {
    if (selectors.has(select)) continue;
    const trigger = button('', () => {
      const label = select.labels?.[0]?.textContent?.trim() || select.getAttribute('aria-label') || 'Choose an option';
      $('choice-title').textContent = label; const list = $('choice-list'); list.replaceChildren(); trigger.setAttribute('aria-expanded','true');
      for (const option of select.options) {
        const row = button('', () => { select.value = option.value; select.dispatchEvent(new Event('change',{bubbles:true})); choiceDialog.close(); updateSelectors(); }, !option.disabled, 'choice-option');
        const text = document.createElement('span'); text.textContent = option.textContent; const check = document.createElement('span'); check.className = 'selection-check'; if (option.selected) check.append(icon('check')); row.append(text,check); row.setAttribute('role','option'); row.setAttribute('aria-selected',String(option.selected)); list.append(row);
      }
      choiceDialog.addEventListener('close', () => { trigger.setAttribute('aria-expanded','false'); trigger.focus(); }, {once:true});
      choiceDialog.showModal(); list.querySelector('[aria-selected=true]')?.focus();
    }, !select.disabled, 'select-trigger');
    trigger.setAttribute('aria-haspopup','dialog'); trigger.setAttribute('aria-expanded','false'); trigger.dataset.select = select.id; select.after(trigger); select.hidden = true; selectors.set(select,trigger);
  }
  for (const [select, trigger] of selectors) {
    if (!select.isConnected) { selectors.delete(select); continue; }
    const label = select.labels?.[0]?.textContent?.trim() || select.getAttribute('aria-label') || 'Selection';
    const value = document.createElement('span'); value.textContent = select.selectedOptions[0]?.textContent || 'Choose'; trigger.replaceChildren(value, icon('caret-down')); trigger.setAttribute('aria-label', label + ': ' + (select.selectedOptions[0]?.textContent || 'Choose'));
    trigger.disabled = select.disabled; trigger.dataset.capable = String(!select.disabled);
  }
}
