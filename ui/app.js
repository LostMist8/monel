/* =========================================================================
   Monel Gateway — shared UI runtime helpers
   Loaded by ui/index.html and page/admin.html.
   No dependencies. Vanilla JS.
   ========================================================================= */

/* ---------- Toast system (replaces window.alert) ----------------------- */
function ensureToastStack() {
  let stack = document.querySelector('.toast-stack');
  if (!stack) {
    stack = document.createElement('div');
    stack.className = 'toast-stack';
    stack.setAttribute('role', 'status');
    stack.setAttribute('aria-live', 'polite');
    document.body.appendChild(stack);
  }
  return stack;
}

/**
 * Show a toast notification.
 * @param {string} message
 * @param {'success'|'error'|'warning'|'info'} [variant='info']
 * @param {number} [duration=3500] ms
 */
function showToast(message, variant = 'info', duration = 3500) {
  const stack = ensureToastStack();
  const toast = document.createElement('div');
  toast.className = `toast toast-${variant}`;
  const iconMap = {
    success: 'check_circle',
    error:   'cancel',
    warning: 'warning',
    info:    'info'
  };
  toast.innerHTML =
    `<span class="ms toast-icon">${iconMap[variant] || 'info'}</span>` +
    `<span class="toast-msg">${escapeHtml(message)}</span>`;
  stack.appendChild(toast);

  const dismiss = () => {
    if (toast.classList.contains('removing')) return;
    toast.classList.add('removing');
    toast.addEventListener('animationend', () => toast.remove(), { once: true });
  };
  if (duration > 0) setTimeout(dismiss, duration);

  // Click to dismiss
  toast.addEventListener('click', dismiss);
  return { dismiss };
}

/* ---------- HTML escape ------------------------------------------------ */
function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text == null ? '' : String(text);
  return div.innerHTML;
}

/* ---------- Grain overlay injector ------------------------------------- */
function ensureGrain() {
  if (document.querySelector('.grain-overlay')) return;
  const grain = document.createElement('div');
  grain.className = 'grain-overlay';
  grain.setAttribute('aria-hidden', 'true');
  document.body.appendChild(grain);
}

/* ---------- Auto-init on DOM ready ------------------------------------- */
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', ensureGrain);
} else {
  ensureGrain();
}

/* Expose globally for inline onclick handlers in legacy pages */
window.MonelUI = { showToast, escapeHtml };
