import express from 'express';

export function authMiddleware(req: any, res: any, next: any) {
  const token = req.headers['authorization'];
  if (!token) {
    return res.status(401).json({ error: 'Unauthorized' });
  }
  next();
}

const router = express.Router();

router.post('/auth/login', (req, res) => {
  console.log('Login attempt');
  res.json({ token: 'jwt-token' });
});

router.post('/auth/logout', (req, res) => {
  res.json({ success: true });
});

export default router;
