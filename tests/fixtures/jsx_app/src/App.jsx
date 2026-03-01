import React, { useState, useEffect } from 'react';

const API_BASE = '/api';

function App() {
  const [user, setUser] = useState(null);
  const [products, setProducts] = useState([]);
  const [cart, setCart] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [loginEmail, setLoginEmail] = useState('');
  const [loginPassword, setLoginPassword] = useState('');

  // ---------------------------------------------------------------------------
  // Authentication
  // ---------------------------------------------------------------------------

  const handleLogin = async (e) => {
    e.preventDefault();
    try {
      console.log(`Login attempt for: ${loginEmail}`);

      const response = await fetch(`${API_BASE}/users/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: loginEmail, password: loginPassword }),
      });

      if (!response.ok) {
        throw new Error('Login failed');
      }

      const data = await response.json();
      setUser(data.user);
      localStorage.setItem('token', data.token);

      console.log(`User logged in: ${data.user.name} (${data.user.email})`);
    } catch (err) {
      console.error(`Login failed for email: ${loginEmail}`, err.message);
      setError('Invalid email or password');
    }
  };

  const handleLogout = () => {
    console.log(`User logged out: ${user?.name} (${user?.email})`);
    setUser(null);
    localStorage.removeItem('token');
  };

  // ---------------------------------------------------------------------------
  // Load products
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const fetchProducts = async () => {
      try {
        const response = await fetch(`${API_BASE}/products?page=1&limit=20`);
        const data = await response.json();
        setProducts(data.products);
        console.log(`Loaded ${data.products.length} products`);
      } catch (err) {
        console.error('Failed to load products:', err.message);
        setError('Failed to load products');
      } finally {
        setLoading(false);
      }
    };

    fetchProducts();
  }, []);

  // ---------------------------------------------------------------------------
  // Load user profile
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const token = localStorage.getItem('token');
    if (!token) return;

    const fetchProfile = async () => {
      try {
        const response = await fetch(`${API_BASE}/users/me`, {
          headers: { Authorization: `Bearer ${token}` },
        });

        if (!response.ok) {
          throw new Error('Session expired');
        }

        const userData = await response.json();
        setUser(userData);
        console.log(`Session restored for: ${userData.name} (${userData.email})`);
      } catch (err) {
        console.log('Session restoration failed, clearing token');
        localStorage.removeItem('token');
      }
    };

    fetchProfile();
  }, []);

  // ---------------------------------------------------------------------------
  // Cart operations
  // ---------------------------------------------------------------------------

  const addToCart = (product) => {
    setCart((prev) => {
      const existing = prev.find((item) => item.id === product.id);
      if (existing) {
        return prev.map((item) =>
          item.id === product.id ? { ...item, quantity: item.quantity + 1 } : item,
        );
      }
      return [...prev, { ...product, quantity: 1 }];
    });
    console.log(`Added to cart: ${product.name} ($${product.price})`);
  };

  const handleCheckout = async () => {
    if (!user) {
      setError('Please log in to checkout');
      return;
    }

    const token = localStorage.getItem('token');

    try {
      console.log(`Checkout initiated by ${user.name} (${user.email}), items: ${cart.length}`);

      // Create order
      const orderResponse = await fetch(`${API_BASE}/orders`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({
          items: cart.map((item) => ({
            productId: item.id,
            quantity: item.quantity,
            price: item.price,
          })),
        }),
      });

      if (!orderResponse.ok) throw new Error('Failed to create order');
      const order = await orderResponse.json();

      // Process payment
      const paymentResponse = await fetch(`${API_BASE}/payments`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({
          orderId: order.id,
          amount: cart.reduce((sum, item) => sum + item.price * item.quantity, 0),
          currency: 'USD',
        }),
      });

      if (!paymentResponse.ok) throw new Error('Payment failed');
      const payment = await paymentResponse.json();

      console.log(`Order completed: ${order.id}, payment: ${payment.paymentId}`);
      setCart([]);
      setError(null);
    } catch (err) {
      console.error(`Checkout failed for user ${user.email}:`, err.message);
      setError('Checkout failed. Please try again.');
    }
  };

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  const cartTotal = cart.reduce((sum, item) => sum + item.price * item.quantity, 0);

  if (loading) {
    return <div className="loading">Loading...</div>;
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>E-Commerce Store</h1>
        <nav>
          {user ? (
            <div className="user-nav">
              <span>Welcome, {user.name}</span>
              <span className="cart-count">Cart ({cart.length})</span>
              <button onClick={handleLogout}>Logout</button>
            </div>
          ) : (
            <form onSubmit={handleLogin} className="login-form">
              <input
                type="email"
                value={loginEmail}
                onChange={(e) => setLoginEmail(e.target.value)}
                placeholder="Email"
                required
              />
              <input
                type="password"
                value={loginPassword}
                onChange={(e) => setLoginPassword(e.target.value)}
                placeholder="Password"
                required
              />
              <button type="submit">Login</button>
            </form>
          )}
        </nav>
      </header>

      {error && <div className="error-banner">{error}</div>}

      <main>
        <section className="products-grid">
          {products.map((product) => (
            <div key={product.id} className="product-card">
              {product.images?.[0] && (
                <img src={product.images[0]} alt={product.name} className="product-image" />
              )}
              <h3>{product.name}</h3>
              <p className="price">${product.price.toFixed(2)}</p>
              <p className="description">{product.description}</p>
              <button onClick={() => addToCart(product)} disabled={!product.inStock}>
                {product.inStock ? 'Add to Cart' : 'Out of Stock'}
              </button>
            </div>
          ))}
        </section>

        {cart.length > 0 && (
          <aside className="cart-sidebar">
            <h2>Shopping Cart</h2>
            {cart.map((item) => (
              <div key={item.id} className="cart-item">
                <span>{item.name}</span>
                <span>x{item.quantity}</span>
                <span>${(item.price * item.quantity).toFixed(2)}</span>
              </div>
            ))}
            <div className="cart-total">
              <strong>Total: ${cartTotal.toFixed(2)}</strong>
            </div>
            <button onClick={handleCheckout} className="checkout-btn">
              Checkout
            </button>
          </aside>
        )}
      </main>
    </div>
  );
}

export default App;
