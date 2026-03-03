import express from 'express';
import { authMiddleware } from '@mono/auth';

const app = express();

app.get('/api/health', (req, res) => {
  res.json({ status: 'ok' });
});

app.get('/api/users', authMiddleware, (req, res) => {
  res.json({ users: [] });
});

app.post('/api/users', authMiddleware, (req, res) => {
  console.log('Creating user');
  res.status(201).json({ id: 1 });
});

app.listen(3000);
