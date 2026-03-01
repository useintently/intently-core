import express from 'express';
import { authMiddleware, rateLimiter } from './middleware';
import { ProductService } from '@shop/products';

const app = express();

app.use(express.json());
app.use(rateLimiter);

// Health & diagnostics
app.get('/health', (req, res) => {
  res.json({ status: 'ok', service: 'gateway' });
});

app.get('/health/ready', (req, res) => {
  res.json({ status: 'ready', uptime: process.uptime() });
});

// Gateway routes — proxy to product service
app.get('/api/v1/products', authMiddleware, async (req, res) => {
  const page = req.query.page || '1';
  console.log(`Listing products, page=${page}, user=${req.headers['x-user-id']}`);
  const products = await ProductService.list(Number(page));
  res.json({ products });
});

app.get('/api/v1/products/:id', async (req, res) => {
  console.log(`Fetching product ${req.params.id}`);
  const product = await ProductService.findById(req.params.id);
  if (!product) {
    res.status(404).json({ error: 'product not found' });
    return;
  }
  res.json({ product });
});

app.post('/api/v1/products', authMiddleware, async (req, res) => {
  console.log(`Creating product: name=${req.body.name}, user_email=${req.body.createdBy}`);
  const product = await ProductService.create(req.body);
  res.status(201).json({ product });
});

// Analytics proxy
app.post('/api/v1/analytics/events', authMiddleware, async (req, res) => {
  const response = await fetch('https://analytics.internal.example.com/api/v1/events', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req.body),
  });
  console.log(`Analytics event forwarded, status=${response.status}`);
  res.json({ accepted: true });
});

app.listen(3000, () => {
  console.log('Gateway listening on port 3000');
});
