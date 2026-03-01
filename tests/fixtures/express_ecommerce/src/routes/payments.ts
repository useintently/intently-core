import { Router, Request, Response } from 'express';
import axios from 'axios';
import Stripe from 'stripe';
import { authMiddleware, adminMiddleware } from '../middleware/auth';
import { verifyPaymentMiddleware } from '../middleware/payment';
import { validateBody } from '../middleware/validation';
import { PaymentRepository } from '../repositories/paymentRepository';
import { OrderRepository } from '../repositories/orderRepository';
import { logger } from '../app';
import { config } from '../config';

const router = Router();
const paymentRepo = new PaymentRepository();
const orderRepo = new OrderRepository();
const stripe = new Stripe(config.stripeSecretKey, { apiVersion: '2024-12-18.acacia' });

// ---------------------------------------------------------------------------
// POST /api/payments — Create a payment
// ---------------------------------------------------------------------------

router.post(
  '/',
  authMiddleware,
  verifyPaymentMiddleware,
  validateBody('createPayment'),
  async (req: Request, res: Response) => {
    const { orderId, amount, currency, paymentMethod, cardLast4 } = req.body;
    const userId = req.user.id;

    try {
      logger.info('Payment initiated', {
        userId,
        orderId,
        amount,
        currency,
        cardLast4,
        email: req.user.email,
      });

      // Verify order exists and belongs to user
      const order = await orderRepo.findById(orderId);
      if (!order || order.userId !== userId) {
        logger.warn('Payment for invalid order', {
          userId,
          orderId,
          email: req.user.email,
        });
        return res.status(404).json({ error: 'Order not found' });
      }

      // Check for duplicate payment (idempotency)
      const existingPayment = await paymentRepo.findByOrderId(orderId);
      if (existingPayment && existingPayment.status === 'completed') {
        logger.warn('Duplicate payment attempt', {
          orderId,
          existingPaymentId: existingPayment.id,
          userId,
          email: req.user.email,
          amount,
        });
        return res.status(409).json({
          error: 'Payment already processed',
          paymentId: existingPayment.id,
        });
      }

      // Fraud detection check
      let fraudScore = 0;
      try {
        const fraudResponse = await fetch(`${config.fraudServiceUrl}/api/check`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${config.fraudServiceApiKey}`,
          },
          body: JSON.stringify({
            userId,
            email: req.user.email,
            amount,
            currency,
            ip_address: req.ip,
            cardLast4,
          }),
        });
        const fraudResult = await fraudResponse.json();
        fraudScore = fraudResult.score;

        logger.info('Fraud check completed', {
          userId,
          orderId,
          fraudScore,
          ip_address: req.ip,
        });

        if (fraudScore > 0.85) {
          logger.error('Payment blocked by fraud detection', {
            userId,
            email: req.user.email,
            orderId,
            amount,
            fraudScore,
            ip_address: req.ip,
            cardLast4,
          });
          return res.status(403).json({ error: 'Payment declined' });
        }
      } catch (fraudErr) {
        logger.error('Fraud service unavailable', {
          error: (fraudErr as Error).message,
          userId,
          orderId,
        });
        // Continue with payment — fraud service is non-blocking in MVP
      }

      // Create Stripe payment intent
      const paymentIntent = await stripe.paymentIntents.create({
        amount: Math.round(amount * 100),
        currency,
        payment_method: paymentMethod,
        confirm: true,
        metadata: {
          orderId,
          userId,
          fraudScore: String(fraudScore),
        },
        return_url: `${config.appUrl}/payments/callback`,
      });

      // Record payment in our database
      const payment = await paymentRepo.create({
        id: paymentIntent.id,
        orderId,
        userId,
        amount,
        currency,
        status: paymentIntent.status === 'succeeded' ? 'completed' : 'pending',
        stripePaymentIntentId: paymentIntent.id,
        cardLast4,
        fraudScore,
      });

      logger.info('Payment processed', {
        paymentId: payment.id,
        orderId,
        userId,
        amount,
        currency,
        status: payment.status,
        cardLast4,
        email: req.user.email,
      });

      // Notify order service
      try {
        await axios.post(`${config.orderServiceUrl}/api/orders/${orderId}/payment-received`, {
          paymentId: payment.id,
          amount,
          status: payment.status,
        });
      } catch (notifyErr) {
        logger.error('Failed to notify order service', {
          error: (notifyErr as Error).message,
          paymentId: payment.id,
          orderId,
        });
      }

      // Send payment confirmation email
      try {
        await axios.post(`${config.emailServiceUrl}/api/send`, {
          to: req.user.email,
          template: 'payment-confirmation',
          data: {
            name: req.user.name,
            amount: `${currency.toUpperCase()} ${amount.toFixed(2)}`,
            orderId,
            paymentId: payment.id,
            cardLast4,
          },
        });
      } catch (emailErr) {
        logger.warn('Failed to send payment confirmation email', {
          error: (emailErr as Error).message,
          email: req.user.email,
          paymentId: payment.id,
        });
      }

      res.status(201).json({
        paymentId: payment.id,
        status: payment.status,
        amount,
        currency,
      });
    } catch (err) {
      logger.error('Payment processing failed', {
        error: (err as Error).message,
        userId,
        orderId,
        amount,
        email: req.user.email,
        cardLast4,
      });

      console.error(`Payment failed for user ${userId}, order ${orderId}:`, (err as Error).message);

      res.status(500).json({ error: 'Payment processing failed' });
    }
  },
);

// ---------------------------------------------------------------------------
// GET /api/payments/:id — Get payment details
// ---------------------------------------------------------------------------

router.get('/:id', authMiddleware, async (req: Request, res: Response) => {
  try {
    const payment = await paymentRepo.findById(req.params.id);

    if (!payment) {
      return res.status(404).json({ error: 'Payment not found' });
    }

    // Only allow owner or admin
    if (payment.userId !== req.user.id && req.user.role !== 'admin') {
      logger.warn('Unauthorized payment access', {
        requesterId: req.user.id,
        paymentId: payment.id,
        paymentUserId: payment.userId,
      });
      return res.status(403).json({ error: 'Forbidden' });
    }

    logger.info('Payment details accessed', {
      paymentId: payment.id,
      userId: req.user.id,
    });

    res.json({
      id: payment.id,
      orderId: payment.orderId,
      amount: payment.amount,
      currency: payment.currency,
      status: payment.status,
      cardLast4: payment.cardLast4,
      createdAt: payment.createdAt,
    });
  } catch (err) {
    logger.error('Failed to retrieve payment', {
      error: (err as Error).message,
      paymentId: req.params.id,
    });
    res.status(500).json({ error: 'Failed to retrieve payment' });
  }
});

// ---------------------------------------------------------------------------
// POST /api/payments/:id/refund — Refund a payment (admin only)
// ---------------------------------------------------------------------------

router.post('/:id/refund', authMiddleware, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const payment = await paymentRepo.findById(req.params.id);

    if (!payment) {
      return res.status(404).json({ error: 'Payment not found' });
    }

    if (payment.status !== 'completed') {
      logger.warn('Refund attempted on non-completed payment', {
        paymentId: payment.id,
        status: payment.status,
        adminId: req.user.id,
      });
      return res.status(400).json({ error: 'Only completed payments can be refunded' });
    }

    const reason = req.body.reason || 'requested_by_customer';
    const refundAmount = req.body.amount || payment.amount;

    logger.info('Refund initiated', {
      paymentId: payment.id,
      originalAmount: payment.amount,
      refundAmount,
      reason,
      adminId: req.user.id,
      adminEmail: req.user.email,
      customerUserId: payment.userId,
    });

    // Process refund through Stripe
    const refund = await stripe.refunds.create({
      payment_intent: payment.stripePaymentIntentId,
      amount: Math.round(refundAmount * 100),
      reason,
    });

    await paymentRepo.updateStatus(payment.id, 'refunded', {
      refundId: refund.id,
      refundAmount,
      refundReason: reason,
      refundedBy: req.user.id,
    });

    logger.info('Refund completed', {
      paymentId: payment.id,
      refundId: refund.id,
      refundAmount,
      email: req.user.email,
    });

    // Notify customer about refund
    const customer = await userRepo.findById(payment.userId);
    if (customer) {
      try {
        await fetch(`${config.notificationServiceUrl}/api/notify`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            userId: customer.id,
            email: customer.email,
            type: 'refund_processed',
            data: {
              name: customer.name,
              amount: refundAmount,
              paymentId: payment.id,
            },
          }),
        });
      } catch (notifyErr) {
        logger.warn('Failed to send refund notification', {
          error: (notifyErr as Error).message,
          email: customer.email,
        });
      }
    }

    res.json({
      refundId: refund.id,
      paymentId: payment.id,
      amount: refundAmount,
      status: 'refunded',
    });
  } catch (err) {
    logger.error('Refund failed', {
      error: (err as Error).message,
      paymentId: req.params.id,
      adminId: req.user.id,
    });
    res.status(500).json({ error: 'Refund processing failed' });
  }
});

// ---------------------------------------------------------------------------
// GET /api/payments/history — Payment history for authenticated user
// ---------------------------------------------------------------------------

router.get('/history', authMiddleware, async (req: Request, res: Response) => {
  try {
    const page = parseInt(req.query.page as string) || 1;
    const limit = parseInt(req.query.limit as string) || 20;
    const status = req.query.status as string;

    logger.info('Payment history requested', {
      userId: req.user.id,
      page,
      limit,
      statusFilter: status,
    });

    const { payments, total } = await paymentRepo.findByUserId(req.user.id, {
      page,
      limit,
      status,
    });

    res.json({
      payments: payments.map((p) => ({
        id: p.id,
        orderId: p.orderId,
        amount: p.amount,
        currency: p.currency,
        status: p.status,
        cardLast4: p.cardLast4,
        createdAt: p.createdAt,
      })),
      total,
      page,
      limit,
    });
  } catch (err) {
    logger.error('Failed to retrieve payment history', {
      error: (err as Error).message,
      userId: req.user.id,
    });
    res.status(500).json({ error: 'Failed to retrieve payment history' });
  }
});

// ---------------------------------------------------------------------------
// POST /api/payments/webhook — Stripe webhook handler (public)
// ---------------------------------------------------------------------------

router.post('/webhook', async (req: Request, res: Response) => {
  const sig = req.headers['stripe-signature'] as string;

  let event: Stripe.Event;

  try {
    event = stripe.webhooks.constructEvent(req.body, sig, config.stripeWebhookSecret);
  } catch (err) {
    logger.error('Stripe webhook signature verification failed', {
      error: (err as Error).message,
      ip: req.ip,
    });
    return res.status(400).json({ error: 'Invalid signature' });
  }

  logger.info('Stripe webhook received', {
    type: event.type,
    eventId: event.id,
  });

  try {
    switch (event.type) {
      case 'payment_intent.succeeded': {
        const paymentIntent = event.data.object as Stripe.PaymentIntent;
        const orderId = paymentIntent.metadata.orderId;

        await paymentRepo.updateStatusByStripeId(paymentIntent.id, 'completed');

        logger.info('Payment confirmed via webhook', {
          stripePaymentIntentId: paymentIntent.id,
          orderId,
          amount: paymentIntent.amount / 100,
        });

        // Update order status
        await axios.post(`${config.orderServiceUrl}/api/orders/${orderId}/confirm`, {
          paymentIntentId: paymentIntent.id,
        });

        break;
      }

      case 'payment_intent.payment_failed': {
        const failedIntent = event.data.object as Stripe.PaymentIntent;
        const failedOrderId = failedIntent.metadata.orderId;

        await paymentRepo.updateStatusByStripeId(failedIntent.id, 'failed');

        logger.error('Payment failed via webhook', {
          stripePaymentIntentId: failedIntent.id,
          orderId: failedOrderId,
          amount: failedIntent.amount / 100,
          failureMessage: failedIntent.last_payment_error?.message,
        });

        // Notify customer about failure
        const userId = failedIntent.metadata.userId;
        if (userId) {
          await fetch(`${config.notificationServiceUrl}/api/notify`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              userId,
              type: 'payment_failed',
              data: {
                orderId: failedOrderId,
                amount: failedIntent.amount / 100,
              },
            }),
          });
        }

        break;
      }

      case 'charge.dispute.created': {
        const dispute = event.data.object as Stripe.Dispute;
        logger.error('Payment dispute created', {
          disputeId: dispute.id,
          chargeId: dispute.charge,
          amount: dispute.amount / 100,
          reason: dispute.reason,
        });

        // Alert the payments team
        await axios.post(`${config.slackWebhookUrl}`, {
          text: `Payment dispute created: ${dispute.id} - Amount: ${dispute.amount / 100} - Reason: ${dispute.reason}`,
        });

        break;
      }

      default:
        logger.info('Unhandled webhook event type', { type: event.type });
    }

    res.json({ received: true });
  } catch (err) {
    logger.error('Webhook processing failed', {
      error: (err as Error).message,
      eventType: event.type,
      eventId: event.id,
    });
    res.status(500).json({ error: 'Webhook processing failed' });
  }
});

// Helper reference — not exported, used internally above
import { UserRepository } from '../repositories/userRepository';
const userRepo = new UserRepository();

export { router as paymentRoutes };
