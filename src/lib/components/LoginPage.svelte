<script lang="ts">
  import { createEventDispatcher, afterUpdate, tick } from 'svelte';
  import { invoke, extractInvokeError } from '$lib/utils/safeInvoke.ts';
  import { t } from '$lib/i18n/index.ts';

  const dispatch = createEventDispatcher<{ success: void }>();

  let password = '';
  let error = '';
  let errorDetail = '';
  let loading = false;
  let passwordInput: HTMLInputElement;
  let inputHasValue = false;

  let showResetForm = false;
  let resetDefaultPassword = '';
  let resetNewPassword = '';
  let resetConfirmPassword = '';
  let resetError = '';
  let resetLoading = false;

  function syncInputState() {
    const v = passwordInput?.value ?? password;
    inputHasValue = !!v && v.trim().length > 0;
  }

  afterUpdate(() => {
    syncInputState();
  });

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      if (showResetForm) {
        handleResetPassword();
      } else {
        handleSubmit();
      }
    }
  }

  function handleInputOrChange() {
    syncInputState();
    if (passwordInput) password = passwordInput.value;
  }

  async function handleSubmit(): Promise<void> {
    const realPassword = passwordInput?.value ?? password;
    if (!realPassword.trim()) {
      error = t('login.passwordRequired');
      errorDetail = '';
      if (passwordInput) passwordInput.focus();
      return;
    }

    if (import.meta.env.DEV && password !== realPassword) {
      console.warn('[LoginPage] 响应式值与 DOM 值不同步！', {
        reactiveLen: password.length,
        domLen: realPassword.length,
        reactiveB64: btoa(unescape(encodeURIComponent(password))),
        domB64: btoa(unescape(encodeURIComponent(realPassword))),
      });
    }

    loading = true;
    error = '';
    errorDetail = '';

    try {
      const matched = await invoke<boolean>('verify_password', { password: realPassword });
      if (matched) {
        dispatch('success');
      } else {
        error = t('login.passwordIncorrect');
        errorDetail = '';
        if (passwordInput) {
          passwordInput.value = '';
          password = '';
          syncInputState();
          passwordInput.focus();
        }
      }
    } catch (e) {
      const parsed = extractInvokeError(e);
      if (parsed.msg.includes('无法连接本地 API') || parsed.msg.includes('localhost API')) {
        error = '无法连接本地 API：请确认应用已启动并正在运行';
      } else if (parsed.msg.includes('回退失败 (4') || parsed.msg.includes('回退失败 (5')) {
        error = t('login.verifyFailed');
      } else {
        error = parsed.msg || t('login.verifyFailed');
      }
      errorDetail = parsed.detail ? parsed.detail : '';
      if (!errorDetail && parsed.msg && error !== parsed.msg) {
        errorDetail = parsed.msg.slice(0, 240);
      }
      console.error('验证密码失败:', e, { parsed });
    } finally {
      loading = false;
    }
  }

  function toggleResetForm() {
    showResetForm = !showResetForm;
    if (showResetForm) {
      resetDefaultPassword = '';
      resetNewPassword = '';
      resetConfirmPassword = '';
      resetError = '';
    }
  }

  async function handleResetPassword(): Promise<void> {
    resetError = '';
    if (!resetDefaultPassword.trim()) {
      resetError = t('login.defaultPasswordIncorrect');
      return;
    }
    if (!resetNewPassword.trim()) {
      resetError = t('settingsGeneral.passwordEmpty');
      return;
    }
    if (resetNewPassword !== resetConfirmPassword) {
      resetError = t('settingsGeneral.passwordMismatch');
      return;
    }

    resetLoading = true;
    try {
      await invoke<void>('change_password', { old_password: resetDefaultPassword, new_password: resetNewPassword });
      showResetForm = false;
      error = '';
      errorDetail = '';
      password = '';
      if (passwordInput) {
        passwordInput.value = '';
        syncInputState();
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes('原密码不正确')) {
        resetError = t('login.defaultPasswordIncorrect');
      } else {
        resetError = msg || t('settingsGeneral.passwordChangeFailed');
      }
    } finally {
      resetLoading = false;
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="login-page">
  <div class="login-card">
    <div class="login-logo">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="w-12 h-12 text-primary-500">
        <path stroke-linecap="round" stroke-linejoin="round" d="M16.5 10.5V6.75a4.5 4.5 0 10-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 002.25-2.25v-6.75a2.25 2.25 0 00-2.25-2.25H6.75a2.25 2.25 0 00-2.25 2.25v6.75a2.25 2.25 0 002.25 2.25z" />
      </svg>
    </div>
    <h1 class="login-title">{t('login.title')}</h1>
    <p class="login-subtitle">{t('login.subtitle')}</p>

    <div class="login-form">
      <div class="login-input-group">
        <input
          type="password"
          bind:value={password}
          bind:this={passwordInput}
          placeholder={t('login.passwordPlaceholder')}
          class="login-input"
          on:keydown={handleKeydown}
          on:input={handleInputOrChange}
          on:change={handleInputOrChange}
          on:paste={handleInputOrChange}
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
        />
      </div>

      {#if error}
        <p class="login-error">{error}</p>
      {/if}
      {#if errorDetail}
        <p class="login-error-detail">{errorDetail}</p>
      {/if}

      <button
        type="button"
        on:click={handleSubmit}
        class="login-button"
        disabled={loading || !inputHasValue}
      >
        {#if loading}
          <svg class="animate-spin h-5 w-5 mr-2" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          {t('login.verifying')}
        {:else}
          {t('login.submit')}
        {/if}
      </button>
    </div>

    <div class="login-footer">
      <button type="button" on:click={toggleResetForm} class="forgot-link">
        {t('login.forgotPassword')}
      </button>
    </div>

    {#if showResetForm}
      <div class="reset-section">
        <h3 class="reset-title">{t('login.resetPasswordTitle')}</h3>
        <p class="reset-hint">{t('login.resetPasswordHint')}</p>

        <div class="reset-form">
          <input
            type="password"
            bind:value={resetDefaultPassword}
            placeholder={t('login.defaultPasswordPlaceholder')}
            class="login-input"
            autocomplete="off"
          />
          <input
            type="password"
            bind:value={resetNewPassword}
            placeholder={t('login.newPasswordPlaceholder')}
            class="login-input"
            autocomplete="off"
          />
          <input
            type="password"
            bind:value={resetConfirmPassword}
            placeholder={t('login.confirmPasswordPlaceholder')}
            class="login-input"
            autocomplete="off"
          />

          {#if resetError}
            <p class="login-error">{resetError}</p>
          {/if}

          <div class="reset-actions">
            <button
              type="button"
              on:click={toggleResetForm}
              class="reset-cancel-btn"
            >
              {t('common.cancel')}
            </button>
            <button
              type="button"
              on:click={handleResetPassword}
              disabled={resetLoading}
              class="reset-confirm-btn"
            >
              {#if resetLoading}
                <svg class="animate-spin h-4 w-4 mr-1" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                </svg>
                {t('login.verifying')}
              {:else}
                {t('common.confirm')}
              {/if}
            </button>
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .login-page {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    width: 100%;
    padding: 1rem;
    background: linear-gradient(135deg, #f0f4ff 0%, #e8eeff 50%, #f5f3ff 100%);
  }

  :global(.dark) .login-page {
    background: linear-gradient(135deg, #0f172a 0%, #1e1b4b 50%, #0f172a 100%);
  }

  .login-card {
    width: 100%;
    max-width: 400px;
    padding: 2.5rem;
    border-radius: 1rem;
    background: white;
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1);
  }

  :global(.dark) .login-card {
    background: #1c2333;
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.3), 0 2px 4px -2px rgba(0, 0, 0, 0.2);
  }

  .login-logo {
    display: flex;
    justify-content: center;
    margin-bottom: 1.5rem;
  }

  .login-title {
    text-align: center;
    font-size: 1.5rem;
    font-weight: 600;
    color: #1e293b;
    margin: 0 0 0.5rem 0;
  }

  :global(.dark) .login-title {
    color: #e2e8f0;
  }

  .login-subtitle {
    text-align: center;
    font-size: 0.875rem;
    color: #64748b;
    margin: 0 0 2rem 0;
  }

  :global(.dark) .login-subtitle {
    color: #94a3b8;
  }

  .login-form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .login-input-group {
    display: flex;
    flex-direction: column;
  }

  .login-input {
    width: 100%;
    padding: 0.75rem 1rem;
    border: 1px solid #e2e8f0;
    border-radius: 0.5rem;
    font-size: 0.9375rem;
    color: #1e293b;
    background: #f8fafc;
    outline: none;
    transition: border-color 0.2s, box-shadow 0.2s;
    box-sizing: border-box;
  }

  .login-input:focus {
    border-color: #6366f1;
    box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.15);
  }

  .login-input:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  :global(.dark) .login-input {
    border-color: #30363d;
    color: #e2e8f0;
    background: #0d1117;
  }

  :global(.dark) .login-input:focus {
    border-color: #6366f1;
    box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.25);
  }

  .login-error {
    font-size: 0.8125rem;
    color: #ef4444;
    margin: 0;
  }

  .login-error-detail {
    font-size: 0.75rem;
    color: #991b1b;
    margin: 0.25rem 0 0 0;
    line-height: 1.35;
    word-break: break-word;
    white-space: pre-wrap;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    opacity: 0.85;
  }

  :global(.dark) .login-error-detail {
    color: #fecaca;
    opacity: 0.8;
  }

  .login-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    padding: 0.75rem 1.5rem;
    border: none;
    border-radius: 0.5rem;
    font-size: 0.9375rem;
    font-weight: 500;
    color: white;
    background: #6366f1;
    cursor: pointer;
    transition: background-color 0.2s, opacity 0.2s;
  }

  .login-button:hover:not(:disabled) {
    background: #4f46e5;
  }

  .login-button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .login-footer {
    text-align: center;
    margin-top: 1.25rem;
  }

  .forgot-link {
    background: none;
    border: none;
    color: #6366f1;
    font-size: 0.8125rem;
    cursor: pointer;
    padding: 0;
    transition: color 0.2s;
  }

  .forgot-link:hover {
    color: #4f46e5;
    text-decoration: underline;
  }

  :global(.dark) .forgot-link {
    color: #818cf8;
  }

  :global(.dark) .forgot-link:hover {
    color: #a5b4fc;
  }

  .reset-section {
    margin-top: 1.5rem;
    padding-top: 1.5rem;
    border-top: 1px solid #e2e8f0;
  }

  :global(.dark) .reset-section {
    border-top-color: #30363d;
  }

  .reset-title {
    font-size: 0.9375rem;
    font-weight: 600;
    color: #1e293b;
    margin: 0 0 0.25rem 0;
  }

  :global(.dark) .reset-title {
    color: #e2e8f0;
  }

  .reset-hint {
    font-size: 0.75rem;
    color: #64748b;
    margin: 0 0 1rem 0;
  }

  :global(.dark) .reset-hint {
    color: #94a3b8;
  }

  .reset-form {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .reset-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.25rem;
  }

  .reset-cancel-btn {
    padding: 0.5rem 1rem;
    font-size: 0.8125rem;
    border-radius: 0.375rem;
    border: 1px solid #e2e8f0;
    background: transparent;
    color: #64748b;
    cursor: pointer;
    transition: background-color 0.2s, color 0.2s;
  }

  .reset-cancel-btn:hover {
    background: #f1f5f9;
    color: #475569;
  }

  :global(.dark) .reset-cancel-btn {
    border-color: #30363d;
    color: #94a3b8;
  }

  :global(.dark) .reset-cancel-btn:hover {
    background: #21262d;
    color: #e6edf3;
  }

  .reset-confirm-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0.5rem 1rem;
    font-size: 0.8125rem;
    border-radius: 0.375rem;
    border: none;
    background: #6366f1;
    color: white;
    cursor: pointer;
    transition: background-color 0.2s, opacity 0.2s;
  }

  .reset-confirm-btn:hover:not(:disabled) {
    background: #4f46e5;
  }

  .reset-confirm-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>