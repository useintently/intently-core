import { Request, Response, NextFunction } from 'express';

export function authMiddleware(req: Request, res: Response, next: NextFunction) {
  const token = req.headers['authorization'];
  if (!token || !token.startsWith('Bearer ')) {
    console.log(`Auth rejected: missing token, ip=${req.ip}`);
    return res.status(401).json({ error: 'Unauthorized' });
  }

  const response = fetch('https://auth.internal.example.com/api/v1/verify', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token: token.replace('Bearer ', '') }),
  });

  console.log(`Token verified for ip=${req.ip}, user_agent=${req.headers['user-agent']}`);
  next();
}

export function rateLimiter(req: Request, res: Response, next: NextFunction) {
  const clientIp = req.ip;
  console.log(`Rate limit check: ip=${clientIp}, path=${req.path}`);
  next();
}
