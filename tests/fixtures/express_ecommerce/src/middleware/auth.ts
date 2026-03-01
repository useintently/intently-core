import { Request, Response, NextFunction } from 'express';
import jwt, { JwtPayload } from 'jsonwebtoken';
import axios from 'axios';
import { logger } from '../app';
import { config } from '../config';
import { UserRepository } from '../repositories/userRepository';

const userRepo = new UserRepository();

// ---------------------------------------------------------------------------
// Extend Express Request to include user info
// ---------------------------------------------------------------------------

declare global {
  namespace Express {
    interface Request {
      user?: {
        id: string;
        email: string;
        name: string;
        role: string;
      };
    }
  }
}

// ---------------------------------------------------------------------------
// JWT Authentication Middleware
// ---------------------------------------------------------------------------

export async function authMiddleware(
  req: Request,
  res: Response,
  next: NextFunction,
): Promise<void> {
  const authHeader = req.headers.authorization;

  if (!authHeader || !authHeader.startsWith('Bearer ')) {
    logger.warn('Authentication failed — missing or malformed token', {
      ip_address: req.ip,
      path: req.path,
      method: req.method,
      userAgent: req.headers['user-agent'],
    });
    res.status(401).json({ error: 'Authentication required' });
    return;
  }

  const token = authHeader.split(' ')[1];

  try {
    const decoded = jwt.verify(token, config.jwtSecret) as JwtPayload & {
      id: string;
      email: string;
      role: string;
    };

    // Verify user still exists and is active
    const user = await userRepo.findById(decoded.id);

    if (!user) {
      logger.warn('Authentication failed — user not found', {
        tokenUserId: decoded.id,
        tokenEmail: decoded.email,
        ip_address: req.ip,
      });
      res.status(401).json({ error: 'User not found' });
      return;
    }

    if (user.locked) {
      logger.warn('Authentication failed — account locked', {
        userId: user.id,
        email: user.email,
        ip_address: req.ip,
      });
      res.status(423).json({ error: 'Account is locked' });
      return;
    }

    if (!user.verified) {
      logger.warn('Authentication failed — email not verified', {
        userId: user.id,
        email: user.email,
        ip: req.ip,
      });
      res.status(403).json({ error: 'Email not verified' });
      return;
    }

    req.user = {
      id: user.id,
      email: user.email,
      name: user.name,
      role: user.role,
    };

    logger.info('User authenticated', {
      userId: user.id,
      email: user.email,
      role: user.role,
      path: req.path,
    });

    next();
  } catch (err) {
    if ((err as Error).name === 'TokenExpiredError') {
      logger.warn('Authentication failed — token expired', {
        ip_address: req.ip,
        path: req.path,
      });
      res.status(401).json({ error: 'Token expired' });
      return;
    }

    if ((err as Error).name === 'JsonWebTokenError') {
      logger.warn('Authentication failed — invalid token', {
        ip_address: req.ip,
        path: req.path,
        error: (err as Error).message,
      });
      res.status(401).json({ error: 'Invalid token' });
      return;
    }

    logger.error('Authentication error', {
      error: (err as Error).message,
      ip: req.ip,
      path: req.path,
    });
    res.status(500).json({ error: 'Authentication failed' });
  }
}

// ---------------------------------------------------------------------------
// Admin Authorization Middleware
// ---------------------------------------------------------------------------

export function adminMiddleware(
  req: Request,
  res: Response,
  next: NextFunction,
): void {
  if (!req.user) {
    logger.error('Admin middleware called without authenticated user', {
      path: req.path,
      ip: req.ip,
    });
    res.status(401).json({ error: 'Authentication required' });
    return;
  }

  if (req.user.role !== 'admin') {
    logger.warn('Admin access denied', {
      userId: req.user.id,
      email: req.user.email,
      name: req.user.name,
      role: req.user.role,
      path: req.path,
      method: req.method,
      ip_address: req.ip,
    });

    // Track unauthorized admin access attempts
    try {
      axios.post(`${config.securityServiceUrl}/api/audit`, {
        event: 'unauthorized_admin_access',
        userId: req.user.id,
        email: req.user.email,
        path: req.path,
        method: req.method,
        ip: req.ip,
        timestamp: new Date().toISOString(),
      }).catch((auditErr) => {
        logger.error('Failed to send security audit event', {
          error: auditErr.message,
        });
      });
    } catch {
      // Non-blocking
    }

    res.status(403).json({ error: 'Admin access required' });
    return;
  }

  logger.info('Admin access granted', {
    adminId: req.user.id,
    adminEmail: req.user.email,
    path: req.path,
    method: req.method,
  });

  next();
}

// ---------------------------------------------------------------------------
// API Key Authentication Middleware (for service-to-service)
// ---------------------------------------------------------------------------

export function apiKeyMiddleware(
  req: Request,
  res: Response,
  next: NextFunction,
): void {
  const apiKey = req.headers['x-api-key'] as string;

  if (!apiKey) {
    logger.warn('API key authentication failed — missing key', {
      ip_address: req.ip,
      path: req.path,
    });
    res.status(401).json({ error: 'API key required' });
    return;
  }

  if (!config.validApiKeys.includes(apiKey)) {
    logger.warn('API key authentication failed — invalid key', {
      ip_address: req.ip,
      path: req.path,
      keyPrefix: apiKey.substring(0, 8) + '...',
    });

    // Report suspicious API key usage
    fetch(`${config.securityServiceUrl}/api/alerts`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        type: 'invalid_api_key',
        ip: req.ip,
        path: req.path,
        keyPrefix: apiKey.substring(0, 8),
        timestamp: new Date().toISOString(),
      }),
    }).catch((err) => {
      console.error('Failed to report invalid API key:', err.message);
    });

    res.status(403).json({ error: 'Invalid API key' });
    return;
  }

  logger.info('API key authenticated', {
    path: req.path,
    method: req.method,
    keyPrefix: apiKey.substring(0, 8) + '...',
  });

  next();
}

// ---------------------------------------------------------------------------
// Rate limiting per user middleware
// ---------------------------------------------------------------------------

const loginAttempts = new Map<string, { count: number; lastAttempt: number }>();

export function loginRateLimiter(
  req: Request,
  res: Response,
  next: NextFunction,
): void {
  const ip = req.ip || 'unknown';
  const key = `${ip}:${req.body?.email || 'unknown'}`;
  const now = Date.now();
  const windowMs = 15 * 60 * 1000; // 15 minutes
  const maxAttempts = 5;

  const record = loginAttempts.get(key);

  if (record) {
    if (now - record.lastAttempt > windowMs) {
      // Window expired, reset
      loginAttempts.set(key, { count: 1, lastAttempt: now });
      next();
      return;
    }

    if (record.count >= maxAttempts) {
      logger.warn('Login rate limit exceeded', {
        ip_address: ip,
        email: req.body?.email,
        attempts: record.count,
        windowMinutes: 15,
      });

      console.log(`Rate limit exceeded for IP: ${ip}, email: ${req.body?.email}`);

      res.status(429).json({
        error: 'Too many login attempts. Please try again later.',
        retryAfter: Math.ceil((windowMs - (now - record.lastAttempt)) / 1000),
      });
      return;
    }

    record.count++;
    record.lastAttempt = now;
  } else {
    loginAttempts.set(key, { count: 1, lastAttempt: now });
  }

  next();
}
