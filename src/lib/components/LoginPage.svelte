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

  function syncInputState() {
    const v = passwordInput?.value ?? password;
    inputHasValue = !!v && v.trim().length > 0;
  }

  // 浏览器 autofill 可能在组件挂载后异步填充 DOM，
  // 此时既不触发 input 事件，也不触发 Svelte bind:value 同步。
  // 因此每次更新后都按 DOM 实际值刷新按钮可用状态。
  afterUpdate(() => {
    syncInputState();
  });

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      handleSubmit();
    }
  }

  function handleInputOrChange() {
    // 无论事件名是什么，立即按 DOM 实际值同步一次
    syncInputState();
    // 顺便让响应式变量 password 追平，便于下游逻辑使用
    if (passwordInput) password = passwordInput.value;
  }

  async function handleSubmit(): Promise<void> {
    // 防范浏览器自动填充与 Svelte bind:value 不同步：
    // 某些情况下 Chrome autofill 直接写入 DOM value 但不派发 input 事件，
    // 导致响应式变量 password 仍为空/旧值，而用户肉眼看到的密码框值其实是正确的。
    // 此时直接读取 passwordInput.value，以 DOM 上真实呈现给用户的字符为准。
    const realPassword = passwordInput?.value ?? password;
    if (!realPassword.trim()) {
      error = t('login.passwordRequired');
      errorDetail = '';
      if (passwordInput) passwordInput.focus();
      return;
    }

    if (import.meta.env.DEV && password !== realPassword) {
      // 开发环境提示：检测到响应式绑定值与 DOM 实际值不一致，说明触发了 autofill 不同步 bug
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
      // 区分几类常见错误，给用户更清晰的指引
      if (parsed.msg.includes('无法连接本地 API') || parsed.msg.includes('localhost API')) {
        error = '无法连接本地 API：请确认应用已启动并正在运行';
      } else if (parsed.msg.includes('回退失败 (4') || parsed.msg.includes('回退失败 (5')) {
        error = t('login.verifyFailed');
      } else {
        error = parsed.msg || t('login.verifyFailed');
      }
      errorDetail = parsed.detail ? parsed.detail : '';
      // 如果没有 detail 但有 msg 内容且不是我们自定义的提示，也把 msg 作为技术细节
      if (!errorDetail && parsed.msg && error !== parsed.msg) {
        errorDetail = parsed.msg.slice(0, 240);
      }
      console.error('验证密码失败:', e, { parsed });
    } finally {
      loading = false;
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

    <p class="login-hint">{t('login.defaultHint')}</p>
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

  .login-hint {
    text-align: center;
    font-size: 0.75rem;
    color: #94a3b8;
    margin: 1.5rem 0 0 0;
  }

  :global(.dark) .login-hint {
    color: #64748b;
  }
</style>