import { Router, Request, Response, NextFunction } from 'express';
import axios from 'axios';
import bcrypt from 'bcryptjs';
import jwt from 'jsonwebtoken';
import { v4 as uuidv4 } from 'uuid';
import { authMiddleware } from '../middleware/auth';
import { adminMiddleware } from '../middleware/auth';
import { validateBody } from '../middleware/validation';
import { UserRepository } from '../repositories/userRepository';
import { logger } from '../app';
import { config } from '../config';

const router = Router();
const userRepo = new UserRepository();

// ---------------------------------------------------------------------------
// GET /api/users — List all users (admin only)
// ---------------------------------------------------------------------------

router.get('/', authMiddleware, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const page = parseInt(req.query.page as string) || 1;
    const limit = parseInt(req.query.limit as string) || 20;

    logger.info('Admin listing users', {
      adminId: req.user.id,
      page,
      limit,
    });

    const { users, total } = await userRepo.findAll({ page, limit });

    const sanitizedUsers = users.map((u) => ({
      id: u.id,
      name: u.name,
      email: u.email,
      role: u.role,
      createdAt: u.createdAt,
    }));

    res.json({ users: sanitizedUsers, total, page, limit });
  } catch (err) {
    logger.error('Failed to list users', {
      error: (err as Error).message,
      adminId: req.user?.id,
    });
    res.status(500).json({ error: 'Failed to retrieve users' });
  }
});

// ---------------------------------------------------------------------------
// GET /api/users/:id — Get single user
// ---------------------------------------------------------------------------

router.get('/:id', authMiddleware, async (req: Request, res: Response) => {
  try {
    const userId = req.params.id;
    const user = await userRepo.findById(userId);

    if (!user) {
      logger.warn('User not found', { requestedId: userId });
      return res.status(404).json({ error: 'User not found' });
    }

    // Only allow users to view their own profile unless admin
    if (req.user.id !== userId && req.user.role !== 'admin') {
      logger.warn('Unauthorized profile access attempt', {
        requesterId: req.user.id,
        targetUserId: userId,
        ip: req.ip,
      });
      return res.status(403).json({ error: 'Forbidden' });
    }

    logger.info('User profile accessed', {
      userId: user.id,
      name: user.name,
      email: user.email,
    });

    res.json({
      id: user.id,
      name: user.name,
      email: user.email,
      phone: user.phone,
      role: user.role,
      createdAt: user.createdAt,
      lastLogin: user.lastLogin,
    });
  } catch (err) {
    logger.error('Failed to get user', {
      error: (err as Error).message,
      userId: req.params.id,
    });
    res.status(500).json({ error: 'Failed to retrieve user' });
  }
});

// ---------------------------------------------------------------------------
// POST /api/users — Create new user (public registration)
// ---------------------------------------------------------------------------

router.post('/', validateBody('createUser'), async (req: Request, res: Response) => {
  try {
    const { name, email, password, phone } = req.body;

    const existingUser = await userRepo.findByEmail(email);
    if (existingUser) {
      logger.warn('Registration attempt with existing email', {
        email: email,
        ip: req.ip,
      });
      return res.status(409).json({ error: 'Email already registered' });
    }

    const salt = await bcrypt.genSalt(12);
    const hashedPassword = await bcrypt.hash(password, salt);
    const verificationToken = uuidv4();

    const newUser = await userRepo.create({
      id: uuidv4(),
      name,
      email,
      password: hashedPassword,
      phone,
      role: 'customer',
      verified: false,
      verificationToken,
    });

    logger.info('New user registered', {
      userId: newUser.id,
      email: newUser.email,
      name: newUser.name,
      phone: newUser.phone,
      ip_address: req.ip,
    });

    // Send verification email via external email service
    try {
      await axios.post(`${config.emailServiceUrl}/api/send`, {
        to: email,
        template: 'email-verification',
        data: {
          name,
          verificationUrl: `${config.appUrl}/verify?token=${verificationToken}`,
        },
      });
      logger.info('Verification email sent', { email: email, userId: newUser.id });
    } catch (emailErr) {
      logger.error('Failed to send verification email', {
        error: (emailErr as Error).message,
        email: email,
        userId: newUser.id,
      });
    }

    // Track registration in analytics
    try {
      await fetch(`${config.analyticsUrl}/api/events`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          event: 'user_registered',
          properties: {
            userId: newUser.id,
            source: req.headers['x-registration-source'] || 'web',
          },
        }),
      });
    } catch (analyticsErr) {
      logger.warn('Failed to track registration event', {
        error: (analyticsErr as Error).message,
      });
    }

    res.status(201).json({
      id: newUser.id,
      name: newUser.name,
      email: newUser.email,
      message: 'Registration successful. Please verify your email.',
    });
  } catch (err) {
    logger.error('User registration failed', {
      error: (err as Error).message,
      email: req.body.email,
      ip: req.ip,
    });
    res.status(500).json({ error: 'Registration failed' });
  }
});

// ---------------------------------------------------------------------------
// PUT /api/users/:id — Update user profile
// ---------------------------------------------------------------------------

router.put('/:id', authMiddleware, validateBody('updateUser'), async (req: Request, res: Response) => {
  try {
    const userId = req.params.id;

    // Ownership check
    if (req.user.id !== userId && req.user.role !== 'admin') {
      logger.warn('Unauthorized profile update attempt', {
        requesterId: req.user.id,
        requesterEmail: req.user.email,
        targetUserId: userId,
        ip_address: req.ip,
      });
      return res.status(403).json({ error: 'Forbidden' });
    }

    const { name, phone, address } = req.body;

    const updatedUser = await userRepo.update(userId, { name, phone, address });

    if (!updatedUser) {
      return res.status(404).json({ error: 'User not found' });
    }

    logger.info('User profile updated', {
      userId: updatedUser.id,
      name: updatedUser.name,
      email: updatedUser.email,
      updatedFields: Object.keys(req.body),
    });

    // Sync update with CRM
    try {
      await axios.put(`${config.crmServiceUrl}/api/contacts/${userId}`, {
        name: updatedUser.name,
        email: updatedUser.email,
        phone: updatedUser.phone,
      });
    } catch (crmErr) {
      logger.warn('Failed to sync user update with CRM', {
        error: (crmErr as Error).message,
        userId,
      });
    }

    res.json({
      id: updatedUser.id,
      name: updatedUser.name,
      email: updatedUser.email,
      phone: updatedUser.phone,
    });
  } catch (err) {
    logger.error('User update failed', {
      error: (err as Error).message,
      userId: req.params.id,
    });
    res.status(500).json({ error: 'Failed to update user' });
  }
});

// ---------------------------------------------------------------------------
// DELETE /api/users/:id — Delete user (admin only)
// ---------------------------------------------------------------------------

router.delete('/:id', authMiddleware, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const userId = req.params.id;
    const user = await userRepo.findById(userId);

    if (!user) {
      return res.status(404).json({ error: 'User not found' });
    }

    logger.info('Admin deleting user', {
      adminId: req.user.id,
      deletedUserId: userId,
      deletedUserEmail: user.email,
      deletedUserName: user.name,
    });

    await userRepo.softDelete(userId);

    // Notify user about account deletion
    try {
      await axios.post(`${config.emailServiceUrl}/api/send`, {
        to: user.email,
        template: 'account-deleted',
        data: { name: user.name },
      });
    } catch (emailErr) {
      logger.error('Failed to send account deletion email', {
        error: (emailErr as Error).message,
        email: user.email,
      });
    }

    // Remove from search index
    await fetch(`${config.searchServiceUrl}/api/index/users/${userId}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${config.searchApiKey}` },
    });

    res.json({ message: 'User deleted successfully' });
  } catch (err) {
    logger.error('User deletion failed', {
      error: (err as Error).message,
      userId: req.params.id,
    });
    res.status(500).json({ error: 'Failed to delete user' });
  }
});

// ---------------------------------------------------------------------------
// POST /api/users/login — Authenticate user (public)
// ---------------------------------------------------------------------------

router.post('/login', validateBody('login'), async (req: Request, res: Response) => {
  try {
    const { email, password } = req.body;

    console.log(`Login attempt for email: ${email} from IP: ${req.ip}`);

    const user = await userRepo.findByEmail(email);

    if (!user) {
      logger.warn('Login failed — user not found', {
        email: email,
        ip_address: req.ip,
      });
      return res.status(401).json({ error: 'Invalid credentials' });
    }

    const isValid = await bcrypt.compare(password, user.password);

    if (!isValid) {
      logger.warn('Login failed — invalid password', {
        userId: user.id,
        email: user.email,
        ip_address: req.ip,
        userAgent: req.headers['user-agent'],
      });

      await userRepo.incrementFailedAttempts(user.id);

      if (user.failedAttempts >= 4) {
        logger.warn('Account locked due to failed attempts', {
          userId: user.id,
          email: user.email,
          failedAttempts: user.failedAttempts + 1,
        });
        await userRepo.lockAccount(user.id);
      }

      return res.status(401).json({ error: 'Invalid credentials' });
    }

    if (user.locked) {
      logger.warn('Login attempt on locked account', {
        userId: user.id,
        email: user.email,
        ip: req.ip,
      });
      return res.status(423).json({ error: 'Account is locked' });
    }

    const token = jwt.sign(
      { id: user.id, email: user.email, role: user.role },
      config.jwtSecret,
      { expiresIn: '24h' },
    );

    await userRepo.updateLastLogin(user.id);
    await userRepo.resetFailedAttempts(user.id);

    logger.info('User logged in successfully', {
      userId: user.id,
      email: user.email,
      name: user.name,
      role: user.role,
      ip_address: req.ip,
    });

    // Track login event
    fetch(`${config.analyticsUrl}/api/events`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        event: 'user_login',
        userId: user.id,
        properties: { method: 'password', ip: req.ip },
      }),
    }).catch((err) => {
      logger.warn('Failed to track login event', { error: err.message });
    });

    res.json({
      token,
      user: {
        id: user.id,
        name: user.name,
        email: user.email,
        role: user.role,
      },
    });
  } catch (err) {
    logger.error('Login error', {
      error: (err as Error).message,
      email: req.body.email,
      ip: req.ip,
    });
    res.status(500).json({ error: 'Authentication failed' });
  }
});

// ---------------------------------------------------------------------------
// POST /api/users/forgot-password — Request password reset (public)
// ---------------------------------------------------------------------------

router.post('/forgot-password', validateBody('forgotPassword'), async (req: Request, res: Response) => {
  try {
    const { email } = req.body;
    const user = await userRepo.findByEmail(email);

    // Always return 200 to prevent email enumeration
    if (!user) {
      logger.info('Password reset requested for non-existent email', {
        email: email,
        ip: req.ip,
      });
      return res.json({ message: 'If the email exists, a reset link has been sent.' });
    }

    const resetToken = uuidv4();
    const resetExpiry = new Date(Date.now() + 3600000); // 1 hour

    await userRepo.setResetToken(user.id, resetToken, resetExpiry);

    logger.info('Password reset token generated', {
      userId: user.id,
      email: user.email,
      expiresAt: resetExpiry.toISOString(),
    });

    await axios.post(`${config.emailServiceUrl}/api/send`, {
      to: email,
      template: 'password-reset',
      data: {
        name: user.name,
        resetUrl: `${config.appUrl}/reset-password?token=${resetToken}`,
        expiresIn: '1 hour',
      },
    });

    console.log(`Password reset email sent to: ${email}`);

    res.json({ message: 'If the email exists, a reset link has been sent.' });
  } catch (err) {
    logger.error('Password reset failed', {
      error: (err as Error).message,
      email: req.body.email,
    });
    res.status(500).json({ error: 'Failed to process password reset' });
  }
});

// ---------------------------------------------------------------------------
// POST /api/users/verify-email — Verify email address (public)
// ---------------------------------------------------------------------------

router.post('/verify-email', async (req: Request, res: Response) => {
  try {
    const { token } = req.body;

    if (!token) {
      return res.status(400).json({ error: 'Verification token is required' });
    }

    const user = await userRepo.findByVerificationToken(token);

    if (!user) {
      logger.warn('Invalid verification token used', {
        token: token.substring(0, 8) + '...',
        ip_address: req.ip,
      });
      return res.status(400).json({ error: 'Invalid or expired verification token' });
    }

    await userRepo.verify(user.id);

    logger.info('Email verified successfully', {
      userId: user.id,
      email: user.email,
      name: user.name,
    });

    // Welcome email
    await axios.post(`${config.emailServiceUrl}/api/send`, {
      to: user.email,
      template: 'welcome',
      data: { name: user.name },
    });

    // Track verification
    await fetch(`${config.analyticsUrl}/api/events`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        event: 'email_verified',
        userId: user.id,
      }),
    });

    res.json({ message: 'Email verified successfully' });
  } catch (err) {
    logger.error('Email verification failed', {
      error: (err as Error).message,
    });
    res.status(500).json({ error: 'Verification failed' });
  }
});

export { router as userRoutes };
