import fetch from 'node-fetch';
import crypto from 'crypto';
import { v4 as uuidv4 } from 'uuid';
import { logger } from '../utils/logger.js';

export class GitHubOAuthService {
  constructor(database) {
    this.db = database;
    this.clientId = process.env.GITHUB_CLIENT_ID;
    this.clientSecret = process.env.GITHUB_CLIENT_SECRET;
    this.callbackUrl = process.env.GITHUB_CALLBACK_URL;
    if (!this.clientId || !this.clientSecret || !this.callbackUrl) {
      logger.warn('GitHub OAuth not fully configured (GITHUB_CLIENT_ID/SECRET/CALLBACK_URL)');
    }
    this.stateStore = new Map();
  }

  isEnabled() {
    return !!(this.clientId && this.clientSecret && this.callbackUrl);
  }

  createAuthUrl() {
    const state = crypto.randomBytes(16).toString('hex');
    this.stateStore.set(state, Date.now());
    const scope = 'read:user user:email';
    const url = `https://github.com/login/oauth/authorize?client_id=${this.clientId}&redirect_uri=${encodeURIComponent(this.callbackUrl)}&scope=${encodeURIComponent(scope)}&state=${state}`;
    return { url, state };
  }

  validateState(state) {
    const created = this.stateStore.get(state);
    if (!created) return false;
    const valid = Date.now() - created < 10 * 60 * 1000; // 10 minutes
    if (!valid) this.stateStore.delete(state);
    return valid;
  }

  async exchangeCodeForToken(code) {
    const res = await fetch('https://github.com/login/oauth/access_token', {
      method: 'POST',
      headers: { 'Accept': 'application/json' },
      body: new URLSearchParams({
        client_id: this.clientId,
        client_secret: this.clientSecret,
        code,
        redirect_uri: this.callbackUrl,
      }),
    });
    if (!res.ok) throw new Error(`GitHub token exchange failed: ${res.status}`);
    const data = await res.json();
    if (!data.access_token) throw new Error('No access_token from GitHub');
    return data.access_token;
  }

  async fetchGitHubUser(accessToken) {
    const res = await fetch('https://api.github.com/user', {
      headers: { 'Authorization': `Bearer ${accessToken}`, 'User-Agent': 'juno-cloud-backend' },
    });
    if (!res.ok) throw new Error(`GitHub user fetch failed: ${res.status}`);
    const user = await res.json();

    // Try to get primary email if email is null/hidden
    let email = user.email;
    if (!email) {
      const er = await fetch('https://api.github.com/user/emails', {
        headers: { 'Authorization': `Bearer ${accessToken}`, 'User-Agent': 'juno-cloud-backend' },
      });
      if (er.ok) {
        const emails = await er.json();
        const primary = emails.find((e) => e.primary) || emails[0];
        email = primary?.email || null;
      }
    }

    return {
      id: user.id,
      login: user.login,
      name: user.name || user.login,
      email,
      avatar_url: user.avatar_url,
    };
  }

  async ensureUser(email, name) {
    if (!email) {
      // Create a placeholder email if not provided by GitHub
      email = `user-${uuidv4()}@users.noreply.github.com`;
    }
    const existing = await this.db.getUserByEmail(email);
    if (existing) {
      if (name && existing.name !== name) {
        await this.db.updateUserName(existing.id, name);
      }
      return existing.id;
    }
    const id = uuidv4();
    await this.db.createUser({ id, email, name });
    return id;
  }
}


