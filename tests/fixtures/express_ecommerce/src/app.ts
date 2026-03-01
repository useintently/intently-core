import express, { Request, Response, NextFunction } from 'express';
import cors from 'cors';
import helmet from 'helmet';
import morgan from 'morgan';
import winston from 'winston';
import { createClient } from 'redis';
import { userRoutes } from './routes/users';
import { paymentRoutes } from './routes/payments';
import { productRoutes } from './routes/products';
import { orderRoutes } from './routes/orders';
import { authMiddleware } from './middleware/auth';
import { rateLimiter } from './middleware/rateLimit';
import { config } from './config';

// ---------------------------------------------------------------------------
// Logger setup
// ---------------------------------------------------------------------------

const logger = winston.createLogger({
  level: config.logLevel || 'info',
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.json(),
  ),
  defaultMeta: { service: 'ecommerce-api' },
  transports: [
    new winston.transports.File({ filename: 'logs/error.log', level: 'error' }),
    new winston.transports.File({ filename: 'logs/combined.log' }),
  ],
});

if (process.env.NODE_ENV !== 'production') {
  logger.add(new winston.transports.Console({
    format: winston.format.simple(),
  }));
}

// ---------------------------------------------------------------------------
// Redis client
// ---------------------------------------------------------------------------

const redisClient = createClient({ url: config.redisUrl });

redisClient.on('error', (err) => {
  logger.error('Redis connection error', { error: err.message });
});

redisClient.on('connect', () => {
  logger.info('Connected to Redis successfully');
});

// ---------------------------------------------------------------------------
// Express application
// ---------------------------------------------------------------------------

const app = express();

// Security headers
app.use(helmet());

// CORS configuration
app.use(cors({
  origin: config.allowedOrigins,
  methods: ['GET', 'POST', 'PUT', 'DELETE', 'PATCH'],
  allowedHeaders: ['Content-Type', 'Authorization', 'X-Request-ID'],
  credentials: true,
}));

// Body parsing
app.use(express.json({ limit: '10mb' }));
app.use(express.urlencoded({ extended: true }));

// Request logging
app.use(morgan('combined', {
  stream: { write: (message: string) => logger.info(message.trim()) },
}));

// Rate limiting
app.use(rateLimiter);

// Request ID middleware
app.use((req: Request, _res: Response, next: NextFunction) => {
  req.headers['x-request-id'] = req.headers['x-request-id'] || crypto.randomUUID();
  logger.info('Incoming request', {
    method: req.method,
    path: req.path,
    requestId: req.headers['x-request-id'],
    ip: req.ip,
  });
  next();
});

// ---------------------------------------------------------------------------
// Health, metrics, and version endpoints
// ---------------------------------------------------------------------------

app.get('/health', (_req: Request, res: Response) => {
  const uptime = process.uptime();
  const memoryUsage = process.memoryUsage();
  logger.info('Health check requested', { uptime });

  res.json({
    status: 'healthy',
    uptime,
    timestamp: new Date().toISOString(),
    memory: {
      heapUsed: memoryUsage.heapUsed,
      heapTotal: memoryUsage.heapTotal,
      rss: memoryUsage.rss,
    },
  });
});

app.get('/metrics', authMiddleware, async (_req: Request, res: Response) => {
  try {
    const activeConnections = await redisClient.dbSize();
    const requestCount = await redisClient.get('metrics:request_count');

    logger.info('Metrics endpoint accessed');

    res.json({
      requests: {
        total: parseInt(requestCount || '0', 10),
      },
      redis: {
        connected: redisClient.isOpen,
        keys: activeConnections,
      },
      process: {
        uptime: process.uptime(),
        pid: process.pid,
        nodeVersion: process.version,
      },
    });
  } catch (err) {
    logger.error('Failed to collect metrics', { error: (err as Error).message });
    res.status(500).json({ error: 'Failed to collect metrics' });
  }
});

app.get('/version', (_req: Request, res: Response) => {
  logger.info('Version endpoint accessed');
  res.json({
    version: config.appVersion,
    environment: config.nodeEnv,
    buildDate: config.buildDate,
    commitSha: config.commitSha,
  });
});

// ---------------------------------------------------------------------------
// Mount route modules
// ---------------------------------------------------------------------------

app.use('/api/users', userRoutes);
app.use('/api/payments', paymentRoutes);
app.use('/api/products', productRoutes);
app.use('/api/orders', orderRoutes);

// ---------------------------------------------------------------------------
// 404 handler
// ---------------------------------------------------------------------------

app.use((_req: Request, res: Response) => {
  logger.warn('Route not found', {
    method: _req.method,
    path: _req.path,
    ip: _req.ip,
  });
  res.status(404).json({ error: 'Not found', path: _req.path });
});

// ---------------------------------------------------------------------------
// Global error handler
// ---------------------------------------------------------------------------

app.use((err: Error, req: Request, res: Response, _next: NextFunction) => {
  const requestId = req.headers['x-request-id'];

  logger.error('Unhandled error', {
    error: err.message,
    stack: err.stack,
    method: req.method,
    path: req.path,
    requestId,
    ip: req.ip,
    userAgent: req.headers['user-agent'],
  });

  if (err.name === 'ValidationError') {
    return res.status(400).json({
      error: 'Validation failed',
      details: err.message,
      requestId,
    });
  }

  if (err.name === 'UnauthorizedError') {
    return res.status(401).json({
      error: 'Authentication required',
      requestId,
    });
  }

  res.status(500).json({
    error: 'Internal server error',
    requestId,
  });
});

// ---------------------------------------------------------------------------
// Server startup
// ---------------------------------------------------------------------------

async function startServer(): Promise<void> {
  try {
    await redisClient.connect();
    logger.info('Redis connected');

    const port = config.port || 3000;
    app.listen(port, () => {
      logger.info(`Server started on port ${port}`, {
        environment: config.nodeEnv,
        version: config.appVersion,
      });
      console.log(`Ecommerce API listening on port ${port}`);
    });
  } catch (err) {
    logger.error('Failed to start server', { error: (err as Error).message });
    console.error('Server startup failed:', (err as Error).message);
    process.exit(1);
  }
}

startServer();

export { app, logger, redisClient };
