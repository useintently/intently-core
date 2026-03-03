import express from 'express';
import { Product, Category, CreateProductInput } from './models';

const router = express.Router();

// Internal product service routes (consumed by gateway)
router.get('/products', async (req, res) => {
  const page = Number(req.query.page) || 1;
  const limit = Number(req.query.limit) || 20;
  console.log(`Product list: page=${page}, limit=${limit}`);

  const response = await fetch(`https://inventory.internal.example.com/api/v1/stock?page=${page}`);
  console.log(`Inventory sync status: ${response.status}`);

  res.json({ products: [], page, limit, total: 0 });
});

router.get('/products/:id', async (req, res) => {
  console.log(`Product detail: id=${req.params.id}`);
  res.json({ product: null });
});

router.post('/products', async (req, res) => {
  const input: CreateProductInput = req.body;
  console.log(`Product created: name=${input.name}, sku=${input.sku}, price=${input.price}`);

  await fetch('https://search.internal.example.com/api/v1/index', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ type: 'product', data: input }),
  });

  res.status(201).json({ product: { ...input, id: 'new-id' } });
});

router.put('/products/:id', async (req, res) => {
  console.log(`Product updated: id=${req.params.id}`);
  res.json({ updated: true });
});

router.delete('/products/:id', async (req, res) => {
  console.log(`Product deleted: id=${req.params.id}`);
  res.json({ deleted: true });
});

router.get('/categories', async (req, res) => {
  console.log('Listing categories');
  res.json({ categories: [] });
});

export class ProductService {
  static async list(page: number): Promise<Product[]> {
    return [];
  }

  static async findById(id: string): Promise<Product | null> {
    return null;
  }

  static async create(input: CreateProductInput): Promise<Product> {
    return { id: 'new', ...input, createdAt: new Date() } as Product;
  }
}

export default router;
