import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/**
 * Production Cloud Connector Frontend Integration
 *
 * This module provides a TypeScript interface for the production cloud connector
 * that enables remote control of the Juno AI Computer Use Agent.
 */

export interface CloudConnectorStatus {
  connected: boolean;
  state: string;
  stats: {
    connected_at?: number;
    total_commands: number;
    successful_commands: number;
    failed_commands: number;
    reconnection_count: number;
    last_heartbeat?: number;
    latency_ms?: number;
  } | null;
}

export interface CloudMessage {
  connectionId: string;
  message: string;
}

export class ProductionCloudConnector {
  private isInitialized = false;
  private statusListeners: ((status: CloudConnectorStatus) => void)[] = [];
  private messageListeners: ((message: CloudMessage) => void)[] = [];

  constructor() {
    this.setupEventListeners();
  }

  /**
   * Initialize the production cloud connector
   */
  async initialize(): Promise<void> {
    if (this.isInitialized) {
      console.warn('[CloudConnector] Already initialized');
      return;
    }

    try {
      await invoke('start_production_cloud_connector');
      this.isInitialized = true;
      console.log('[CloudConnector] Production cloud connector started successfully');
    } catch (error) {
      console.error('[CloudConnector] Failed to start production cloud connector:', error);
      throw error;
    }
  }

  /**
   * Stop the production cloud connector
   */
  async stop(): Promise<void> {
    if (!this.isInitialized) {
      console.warn('[CloudConnector] Not initialized');
      return;
    }

    try {
      await invoke('stop_production_cloud_connector');
      this.isInitialized = false;
      console.log('[CloudConnector] Production cloud connector stopped successfully');
    } catch (error) {
      console.error('[CloudConnector] Failed to stop production cloud connector:', error);
      throw error;
    }
  }

  /**
   * Get current connection status
   */
  async getStatus(): Promise<CloudConnectorStatus> {
    try {
      const status = await invoke<CloudConnectorStatus>('get_production_cloud_status');
      return status;
    } catch (error) {
      console.error('[CloudConnector] Failed to get status:', error);
      throw error;
    }
  }

  /**
   * Check if connector is connected and ready
   */
  async isConnected(): Promise<boolean> {
    try {
      const status = await this.getStatus();
      return status.connected && status.state === 'Ready';
    } catch (error) {
      console.error('[CloudConnector] Failed to check connection status:', error);
      return false;
    }
  }

  /**
   * Setup event listeners for cloud connector events
   */
  private async setupEventListeners(): Promise<void> {
    // Listen for connection state changes
    await listen('cloud-connector-state', (event) => {
      console.log('[CloudConnector] State changed:', event.payload);
      this.notifyStatusListeners();
    });

    // Listen for cloud messages (handled by the Rust backend)
    await listen('cloud-message-received', (event) => {
      const message = event.payload as CloudMessage;
      console.log('[CloudConnector] Received cloud message:', message);
      this.messageListeners.forEach(listener => listener(message));
    });

    // Listen for connection errors
    await listen('cloud-connector-error', (event) => {
      console.error('[CloudConnector] Connection error:', event.payload);
    });
  }

  /**
   * Add a status change listener
   */
  onStatusChange(listener: (status: CloudConnectorStatus) => void): () => void {
    this.statusListeners.push(listener);

    // Return unsubscribe function
    return () => {
      const index = this.statusListeners.indexOf(listener);
      if (index > -1) {
        this.statusListeners.splice(index, 1);
      }
    };
  }

  /**
   * Add a cloud message listener
   */
  onMessage(listener: (message: CloudMessage) => void): () => void {
    this.messageListeners.push(listener);

    // Return unsubscribe function
    return () => {
      const index = this.messageListeners.indexOf(listener);
      if (index > -1) {
        this.messageListeners.splice(index, 1);
      }
    };
  }

  /**
   * Notify all status listeners of status changes
   */
  private async notifyStatusListeners(): Promise<void> {
    try {
      const status = await this.getStatus();
      this.statusListeners.forEach(listener => listener(status));
    } catch (error) {
      console.error('[CloudConnector] Failed to notify status listeners:', error);
    }
  }

  /**
   * Get connection statistics for monitoring
   */
  async getConnectionStats(): Promise<CloudConnectorStatus['stats']> {
    try {
      const status = await this.getStatus();
      return status.stats;
    } catch (error) {
      console.error('[CloudConnector] Failed to get connection stats:', error);
      return null;
    }
  }
}

/**
 * Example usage of the Production Cloud Connector
 */
export class CloudConnectorExample {
  private connector: ProductionCloudConnector;
  private statusUnsubscribe?: () => void;
  private messageUnsubscribe?: () => void;

  constructor() {
    this.connector = new ProductionCloudConnector();
  }

  /**
   * Initialize and setup the cloud connector with monitoring
   */
  async setup(): Promise<void> {
    try {
      // Initialize the connector
      await this.connector.initialize();
      console.log('[Example] Cloud connector initialized');

      // Setup status monitoring
      this.statusUnsubscribe = this.connector.onStatusChange((status) => {
        console.log('[Example] Status update:', status);

        if (status.connected) {
          console.log('[Example] ✅ Connected to cloud - remote control is available');
        } else {
          console.log('[Example] ❌ Disconnected from cloud - remote control unavailable');
        }
      });

      // Setup message monitoring
      this.messageUnsubscribe = this.connector.onMessage((message) => {
        console.log('[Example] Received cloud message:', message);
      });

      // Check initial status
      const status = await this.connector.getStatus();
      console.log('[Example] Initial status:', status);

    } catch (error) {
      console.error('[Example] Failed to setup cloud connector:', error);
    }
  }

  /**
   * Cleanup resources
   */
  async cleanup(): Promise<void> {
    // Unsubscribe from events
    if (this.statusUnsubscribe) {
      this.statusUnsubscribe();
    }
    if (this.messageUnsubscribe) {
      this.messageUnsubscribe();
    }

    // Stop the connector
    await this.connector.stop();
    console.log('[Example] Cloud connector cleaned up');
  }

  /**
   * Monitor connection health
   */
  async startHealthMonitoring(): Promise<void> {
    setInterval(async () => {
      try {
        const stats = await this.connector.getConnectionStats();
        if (stats) {
          console.log('[Example] Health check:', {
            totalCommands: stats.total_commands,
            successRate: stats.total_commands > 0
              ? (stats.successful_commands / stats.total_commands * 100).toFixed(1) + '%'
              : 'N/A',
            reconnectionCount: stats.reconnection_count,
            lastHeartbeat: stats.last_heartbeat ? new Date(stats.last_heartbeat * 1000) : 'N/A'
          });
        }
      } catch (error) {
        console.error('[Example] Health check failed:', error);
      }
    }, 30000); // Check every 30 seconds
  }
}

// Global instance for easy access
export const cloudConnector = new ProductionCloudConnector();

/**
 * Auto-initialize cloud connector when module is imported
 * (Optional - you may want to control initialization manually)
 */
export async function autoInitialize(): Promise<void> {
  try {
    console.log('[CloudConnector] Auto-initializing production cloud connector...');
    await cloudConnector.initialize();
    console.log('[CloudConnector] ✅ Production cloud connector ready for remote control');
  } catch (error) {
    console.warn('[CloudConnector] ⚠️ Auto-initialization failed (this is normal if cloud is disabled):', error);
  }
}
