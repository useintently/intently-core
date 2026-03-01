import React, { useState, useEffect, useCallback } from 'react';
import axios from 'axios';

interface User {
  id: string;
  name: string;
  email: string;
  role: string;
  lastLogin: string;
}

interface Payment {
  id: string;
  orderId: string;
  amount: number;
  currency: string;
  status: string;
  cardLast4: string;
  createdAt: string;
}

interface DashboardStats {
  totalUsers: number;
  totalRevenue: number;
  pendingPayments: number;
  activeOrders: number;
}

interface DashboardProps {
  sessionToken: string;
  adminEmail: string;
}

const Dashboard: React.FC<DashboardProps> = ({ sessionToken, adminEmail }) => {
  const [users, setUsers] = useState<User[]>([]);
  const [payments, setPayments] = useState<Payment[]>([]);
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedUser, setSelectedUser] = useState<User | null>(null);
  const [searchQuery, setSearchQuery] = useState<string>('');

  const apiHeaders = {
    Authorization: `Bearer ${sessionToken}`,
    'Content-Type': 'application/json',
  };

  // ---------------------------------------------------------------------------
  // Fetch dashboard data
  // ---------------------------------------------------------------------------

  const fetchDashboardData = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      console.log(`Loading dashboard for admin: ${adminEmail}`);

      // Fetch users
      const usersResponse = await axios.get('/api/users', {
        headers: apiHeaders,
        params: { page: 1, limit: 50 },
      });
      setUsers(usersResponse.data.users);

      console.log(`Loaded ${usersResponse.data.total} users`);

      // Fetch recent payments
      const paymentsResponse = await axios.get('/api/payments/history', {
        headers: apiHeaders,
        params: { page: 1, limit: 20 },
      });
      setPayments(paymentsResponse.data.payments);

      // Fetch stats
      const statsResponse = await axios.get('/api/admin/stats', {
        headers: apiHeaders,
      });
      setStats(statsResponse.data);

      console.log('Dashboard data loaded successfully', {
        users: usersResponse.data.total,
        payments: paymentsResponse.data.total,
      });
    } catch (err) {
      const errorMessage = (err as Error).message;
      setError(errorMessage);
      console.error(`Dashboard data fetch failed for ${adminEmail}:`, errorMessage);
    } finally {
      setLoading(false);
    }
  }, [sessionToken, adminEmail]);

  useEffect(() => {
    fetchDashboardData();
  }, [fetchDashboardData]);

  // ---------------------------------------------------------------------------
  // User actions
  // ---------------------------------------------------------------------------

  const handleDeleteUser = async (userId: string, userEmail: string) => {
    if (!window.confirm(`Are you sure you want to delete user ${userEmail}?`)) {
      return;
    }

    try {
      await axios.delete(`/api/users/${userId}`, { headers: apiHeaders });
      console.log(`User deleted: ${userEmail} (ID: ${userId})`);
      setUsers((prev) => prev.filter((u) => u.id !== userId));
    } catch (err) {
      console.error(`Failed to delete user ${userEmail}:`, (err as Error).message);
      setError(`Failed to delete user: ${(err as Error).message}`);
    }
  };

  const handleViewUser = async (userId: string) => {
    try {
      const response = await axios.get(`/api/users/${userId}`, {
        headers: apiHeaders,
      });
      setSelectedUser(response.data);
      console.log(`Viewing user profile: ${response.data.name} (${response.data.email})`);
    } catch (err) {
      console.error('Failed to fetch user details:', (err as Error).message);
    }
  };

  const handleRefundPayment = async (paymentId: string, amount: number) => {
    try {
      const response = await axios.post(
        `/api/payments/${paymentId}/refund`,
        { reason: 'admin_request', amount },
        { headers: apiHeaders },
      );
      console.log(`Refund processed: ${response.data.refundId} for payment ${paymentId}`);

      setPayments((prev) =>
        prev.map((p) => (p.id === paymentId ? { ...p, status: 'refunded' } : p)),
      );
    } catch (err) {
      console.error(`Refund failed for payment ${paymentId}:`, (err as Error).message);
      setError(`Refund failed: ${(err as Error).message}`);
    }
  };

  // ---------------------------------------------------------------------------
  // Search
  // ---------------------------------------------------------------------------

  const handleSearch = async () => {
    if (!searchQuery.trim()) return;

    try {
      const response = await fetch(
        `/api/admin/search?q=${encodeURIComponent(searchQuery)}`,
        { headers: apiHeaders },
      );
      const data = await response.json();
      console.log(`Search results for "${searchQuery}": ${data.total} matches`);
      setUsers(data.users || []);
    } catch (err) {
      console.error('Search failed:', (err as Error).message);
    }
  };

  // ---------------------------------------------------------------------------
  // Export
  // ---------------------------------------------------------------------------

  const handleExportUsers = async () => {
    try {
      const response = await fetch('/api/admin/export/users', {
        headers: apiHeaders,
      });
      const blob = await response.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'users-export.csv';
      a.click();
      console.log(`Users exported by admin: ${adminEmail}`);
    } catch (err) {
      console.error('Export failed:', (err as Error).message);
    }
  };

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen bg-gray-50 dark:bg-gray-900">
        <div className="text-lg text-gray-600 dark:text-gray-300">Loading dashboard...</div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900 p-6">
      <header className="mb-8">
        <h1 className="text-3xl font-bold text-gray-900 dark:text-white">Admin Dashboard</h1>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">Logged in as {adminEmail}</p>
      </header>

      {error && (
        <div className="mb-6 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p className="text-red-700 dark:text-red-400">{error}</p>
        </div>
      )}

      {stats && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-8">
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm text-gray-500 dark:text-gray-400">Total Users</h3>
            <p className="text-2xl font-bold text-gray-900 dark:text-white">{stats.totalUsers}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm text-gray-500 dark:text-gray-400">Total Revenue</h3>
            <p className="text-2xl font-bold text-green-600">${stats.totalRevenue.toFixed(2)}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm text-gray-500 dark:text-gray-400">Pending Payments</h3>
            <p className="text-2xl font-bold text-yellow-600">{stats.pendingPayments}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm text-gray-500 dark:text-gray-400">Active Orders</h3>
            <p className="text-2xl font-bold text-blue-600">{stats.activeOrders}</p>
          </div>
        </div>
      )}

      <div className="mb-6 flex gap-4">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search users..."
          className="flex-1 px-4 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-600"
        />
        <button onClick={handleSearch} className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700">
          Search
        </button>
        <button onClick={handleExportUsers} className="px-4 py-2 bg-gray-600 text-white rounded-lg hover:bg-gray-700">
          Export CSV
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
          <h2 className="text-xl font-semibold p-4 border-b dark:border-gray-700 text-gray-900 dark:text-white">
            Users
          </h2>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="bg-gray-50 dark:bg-gray-700 text-left text-sm text-gray-500 dark:text-gray-400">
                  <th className="p-3">Name</th>
                  <th className="p-3">Email</th>
                  <th className="p-3">Role</th>
                  <th className="p-3">Actions</th>
                </tr>
              </thead>
              <tbody>
                {users.map((user) => (
                  <tr key={user.id} className="border-t dark:border-gray-700">
                    <td className="p-3 text-gray-900 dark:text-gray-200">{user.name}</td>
                    <td className="p-3 text-gray-600 dark:text-gray-400">{user.email}</td>
                    <td className="p-3">
                      <span
                        className={`px-2 py-1 rounded-full text-xs ${
                          user.role === 'admin'
                            ? 'bg-purple-100 text-purple-700'
                            : 'bg-gray-100 text-gray-700'
                        }`}
                      >
                        {user.role}
                      </span>
                    </td>
                    <td className="p-3">
                      <button
                        onClick={() => handleViewUser(user.id)}
                        className="text-blue-600 hover:underline mr-3"
                      >
                        View
                      </button>
                      <button
                        onClick={() => handleDeleteUser(user.id, user.email)}
                        className="text-red-600 hover:underline"
                      >
                        Delete
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>

        <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
          <h2 className="text-xl font-semibold p-4 border-b dark:border-gray-700 text-gray-900 dark:text-white">
            Recent Payments
          </h2>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="bg-gray-50 dark:bg-gray-700 text-left text-sm text-gray-500 dark:text-gray-400">
                  <th className="p-3">Order</th>
                  <th className="p-3">Amount</th>
                  <th className="p-3">Status</th>
                  <th className="p-3">Actions</th>
                </tr>
              </thead>
              <tbody>
                {payments.map((payment) => (
                  <tr key={payment.id} className="border-t dark:border-gray-700">
                    <td className="p-3 text-gray-900 dark:text-gray-200">{payment.orderId}</td>
                    <td className="p-3 text-gray-600 dark:text-gray-400">
                      {payment.currency} {payment.amount.toFixed(2)}
                    </td>
                    <td className="p-3">
                      <span
                        className={`px-2 py-1 rounded-full text-xs ${
                          payment.status === 'completed'
                            ? 'bg-green-100 text-green-700'
                            : payment.status === 'refunded'
                              ? 'bg-red-100 text-red-700'
                              : 'bg-yellow-100 text-yellow-700'
                        }`}
                      >
                        {payment.status}
                      </span>
                    </td>
                    <td className="p-3">
                      {payment.status === 'completed' && (
                        <button
                          onClick={() => handleRefundPayment(payment.id, payment.amount)}
                          className="text-red-600 hover:underline"
                        >
                          Refund
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>

      {selectedUser && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 max-w-md w-full">
            <h3 className="text-xl font-semibold text-gray-900 dark:text-white mb-4">User Details</h3>
            <dl className="space-y-2">
              <div>
                <dt className="text-sm text-gray-500">Name</dt>
                <dd className="text-gray-900 dark:text-gray-200">{selectedUser.name}</dd>
              </div>
              <div>
                <dt className="text-sm text-gray-500">Email</dt>
                <dd className="text-gray-900 dark:text-gray-200">{selectedUser.email}</dd>
              </div>
              <div>
                <dt className="text-sm text-gray-500">Role</dt>
                <dd className="text-gray-900 dark:text-gray-200">{selectedUser.role}</dd>
              </div>
              <div>
                <dt className="text-sm text-gray-500">Last Login</dt>
                <dd className="text-gray-900 dark:text-gray-200">{selectedUser.lastLogin}</dd>
              </div>
            </dl>
            <button
              onClick={() => setSelectedUser(null)}
              className="mt-6 w-full px-4 py-2 bg-gray-600 text-white rounded-lg hover:bg-gray-700"
            >
              Close
            </button>
          </div>
        </div>
      )}
    </div>
  );
};

export default Dashboard;
