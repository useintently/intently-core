import axios, { AxiosInstance, AxiosError } from 'axios';
import { logger } from '../app';
import { config } from '../config';

// ---------------------------------------------------------------------------
// Stripe service — wraps all Stripe API interactions
// ---------------------------------------------------------------------------

interface StripeCharge {
  id: string;
  amount: number;
  currency: string;
  status: string;
  customer: string;
}

interface StripeRefund {
  id: string;
  amount: number;
  status: string;
  charge: string;
}

interface StripeCustomer {
  id: string;
  email: string;
  name: string;
  defaultPaymentMethod: string | null;
}

interface StripeBalance {
  available: Array<{ amount: number; currency: string }>;
  pending: Array<{ amount: number; currency: string }>;
}

class StripeService {
  private client: AxiosInstance;
  private readonly baseUrl = 'https://api.stripe.com/v1';
  private readonly maxRetries = 3;

  constructor() {
    this.client = axios.create({
      baseURL: this.baseUrl,
      headers: {
        Authorization: `Bearer ${config.stripeSecretKey}`,
        'Content-Type': 'application/x-www-form-urlencoded',
      },
      timeout: 30000,
    });

    // Request interceptor for logging
    this.client.interceptors.request.use((reqConfig) => {
      logger.info('Stripe API request', {
        method: reqConfig.method?.toUpperCase(),
        url: reqConfig.url,
        baseURL: reqConfig.baseURL,
      });
      return reqConfig;
    });

    // Response interceptor for logging
    this.client.interceptors.response.use(
      (response) => {
        logger.info('Stripe API response', {
          status: response.status,
          url: response.config.url,
        });
        return response;
      },
      (error: AxiosError) => {
        logger.error('Stripe API error', {
          status: error.response?.status,
          url: error.config?.url,
          message: error.message,
          stripeError: (error.response?.data as any)?.error?.message,
        });
        throw error;
      },
    );
  }

  // -------------------------------------------------------------------------
  // Create a charge
  // -------------------------------------------------------------------------

  async createCharge(params: {
    amount: number;
    currency: string;
    customerId: string;
    paymentMethodId: string;
    description: string;
    metadata?: Record<string, string>;
  }): Promise<StripeCharge> {
    const { amount, currency, customerId, paymentMethodId, description, metadata } = params;

    logger.info('Creating Stripe charge', {
      amount,
      currency,
      customerId,
      description,
    });

    try {
      const response = await this.client.post('/charges', new URLSearchParams({
        amount: String(Math.round(amount * 100)),
        currency,
        customer: customerId,
        payment_method: paymentMethodId,
        description,
        ...(metadata ? Object.fromEntries(
          Object.entries(metadata).map(([k, v]) => [`metadata[${k}]`, v]),
        ) : {}),
      }));

      const charge = response.data;

      logger.info('Stripe charge created', {
        chargeId: charge.id,
        amount: charge.amount / 100,
        currency: charge.currency,
        customerId,
        status: charge.status,
      });

      return {
        id: charge.id,
        amount: charge.amount / 100,
        currency: charge.currency,
        status: charge.status,
        customer: charge.customer,
      };
    } catch (err) {
      const axiosErr = err as AxiosError;
      const stripeError = (axiosErr.response?.data as any)?.error;

      logger.error('Failed to create Stripe charge', {
        error: stripeError?.message || axiosErr.message,
        type: stripeError?.type,
        code: stripeError?.code,
        customerId,
        amount,
        currency,
      });

      throw new Error(`Stripe charge failed: ${stripeError?.message || axiosErr.message}`);
    }
  }

  // -------------------------------------------------------------------------
  // Create a refund
  // -------------------------------------------------------------------------

  async createRefund(params: {
    chargeId: string;
    amount?: number;
    reason?: string;
  }): Promise<StripeRefund> {
    const { chargeId, amount, reason } = params;

    logger.info('Creating Stripe refund', {
      chargeId,
      amount,
      reason,
    });

    try {
      const formData = new URLSearchParams({ charge: chargeId });
      if (amount) formData.append('amount', String(Math.round(amount * 100)));
      if (reason) formData.append('reason', reason);

      const response = await this.client.post('/refunds', formData);
      const refund = response.data;

      logger.info('Stripe refund created', {
        refundId: refund.id,
        chargeId,
        amount: refund.amount / 100,
        status: refund.status,
      });

      return {
        id: refund.id,
        amount: refund.amount / 100,
        status: refund.status,
        charge: refund.charge,
      };
    } catch (err) {
      const axiosErr = err as AxiosError;
      logger.error('Failed to create Stripe refund', {
        error: axiosErr.message,
        chargeId,
        amount,
      });
      throw new Error(`Stripe refund failed: ${axiosErr.message}`);
    }
  }

  // -------------------------------------------------------------------------
  // Get account balance
  // -------------------------------------------------------------------------

  async getBalance(): Promise<StripeBalance> {
    logger.info('Fetching Stripe balance');

    try {
      const response = await this.client.get('/balance');

      logger.info('Stripe balance retrieved', {
        available: response.data.available,
        pending: response.data.pending,
      });

      return {
        available: response.data.available.map((b: any) => ({
          amount: b.amount / 100,
          currency: b.currency,
        })),
        pending: response.data.pending.map((b: any) => ({
          amount: b.amount / 100,
          currency: b.currency,
        })),
      };
    } catch (err) {
      logger.error('Failed to fetch Stripe balance', {
        error: (err as Error).message,
      });
      throw new Error(`Failed to get Stripe balance: ${(err as Error).message}`);
    }
  }

  // -------------------------------------------------------------------------
  // Create a customer
  // -------------------------------------------------------------------------

  async createCustomer(params: {
    email: string;
    name: string;
    phone?: string;
    metadata?: Record<string, string>;
  }): Promise<StripeCustomer> {
    const { email, name, phone, metadata } = params;

    logger.info('Creating Stripe customer', {
      email: email,
      name: name,
      phone: phone,
    });

    try {
      const formData = new URLSearchParams({ email, name });
      if (phone) formData.append('phone', phone);
      if (metadata) {
        Object.entries(metadata).forEach(([k, v]) => {
          formData.append(`metadata[${k}]`, v);
        });
      }

      const response = await this.client.post('/customers', formData);
      const customer = response.data;

      logger.info('Stripe customer created', {
        customerId: customer.id,
        email: customer.email,
        name: customer.name,
      });

      return {
        id: customer.id,
        email: customer.email,
        name: customer.name,
        defaultPaymentMethod: customer.invoice_settings?.default_payment_method || null,
      };
    } catch (err) {
      const axiosErr = err as AxiosError;
      logger.error('Failed to create Stripe customer', {
        error: axiosErr.message,
        email: email,
        name: name,
      });
      throw new Error(`Stripe customer creation failed: ${axiosErr.message}`);
    }
  }

  // -------------------------------------------------------------------------
  // Get customer by ID
  // -------------------------------------------------------------------------

  async getCustomer(customerId: string): Promise<StripeCustomer> {
    logger.info('Fetching Stripe customer', { customerId });

    try {
      const response = await this.client.get(`/customers/${customerId}`);
      const customer = response.data;

      logger.info('Stripe customer retrieved', {
        customerId: customer.id,
        email: customer.email,
      });

      return {
        id: customer.id,
        email: customer.email,
        name: customer.name,
        defaultPaymentMethod: customer.invoice_settings?.default_payment_method || null,
      };
    } catch (err) {
      logger.error('Failed to fetch Stripe customer', {
        error: (err as Error).message,
        customerId,
      });
      throw new Error(`Failed to get Stripe customer: ${(err as Error).message}`);
    }
  }

  // -------------------------------------------------------------------------
  // List charges for a customer
  // -------------------------------------------------------------------------

  async listCustomerCharges(customerId: string, limit: number = 10): Promise<StripeCharge[]> {
    logger.info('Listing customer charges', { customerId, limit });

    try {
      const response = await this.client.get('/charges', {
        params: { customer: customerId, limit },
      });

      const charges = response.data.data.map((c: any) => ({
        id: c.id,
        amount: c.amount / 100,
        currency: c.currency,
        status: c.status,
        customer: c.customer,
      }));

      logger.info('Customer charges retrieved', {
        customerId,
        count: charges.length,
      });

      return charges;
    } catch (err) {
      logger.error('Failed to list customer charges', {
        error: (err as Error).message,
        customerId,
      });
      throw new Error(`Failed to list charges: ${(err as Error).message}`);
    }
  }

  // -------------------------------------------------------------------------
  // Verify webhook signature
  // -------------------------------------------------------------------------

  verifyWebhookSignature(payload: string, signature: string): boolean {
    try {
      const expectedSig = this.computeSignature(payload, config.stripeWebhookSecret);
      const isValid = signature === expectedSig;

      if (!isValid) {
        logger.warn('Invalid Stripe webhook signature', {
          receivedSignature: signature.substring(0, 20) + '...',
        });
      }

      return isValid;
    } catch (err) {
      logger.error('Webhook signature verification error', {
        error: (err as Error).message,
      });
      return false;
    }
  }

  private computeSignature(payload: string, secret: string): string {
    const crypto = require('crypto');
    return crypto.createHmac('sha256', secret).update(payload).digest('hex');
  }
}

export const stripeService = new StripeService();
export { StripeService };
