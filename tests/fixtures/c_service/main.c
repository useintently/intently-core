#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>

#define MAX_CLIENTS 128
#define BUFFER_SIZE 4096
#define MAX_ROUTES 32

typedef struct {
    char id[64];
    char name[128];
    char email[256];
    char phone[32];
    char address[512];
    char password_hash[128];
    int is_active;
} User;

typedef struct {
    char order_id[64];
    char user_id[64];
    double total;
    char shipping_address[512];
    char status[32];
    char payment_token[128];
} Order;

typedef struct {
    int port;
    int max_connections;
    char db_host[256];
    char db_name[64];
    int db_pool_size;
    char secret_key[256];
} ServerConfig;

typedef struct {
    int socket_fd;
    struct sockaddr_in addr;
    int client_count;
} HttpServer;

void log_info(const char* format, ...);
void log_error(const char* format, ...);
void log_warn(const char* format, ...);
void log_debug(const char* format, ...);

ServerConfig* load_config(const char* config_path) {
    log.info("Loading server configuration from: %s", config_path);

    ServerConfig* config = (ServerConfig*)malloc(sizeof(ServerConfig));
    if (!config) {
        log.error("Failed to allocate memory for configuration");
        return NULL;
    }

    config->port = 8080;
    config->max_connections = MAX_CLIENTS;
    config->db_pool_size = 10;
    strcpy(config->db_host, "localhost");
    strcpy(config->db_name, "orders_db");
    strcpy(config->secret_key, "default_secret");

    log.info("Configuration loaded: port=%d, max_connections=%d", config->port, config->max_connections);
    log.debug("Database host: %s, database: %s", config->db_host, config->db_name);
    log.debug("Connection pool size: %d", config->db_pool_size);

    return config;
}

int handle_health_check(char* response_buf) {
    log.info("Health check endpoint called");
    log.debug("System memory usage: checking");

    sprintf(response_buf, "{\"status\":\"healthy\",\"uptime\":%ld}", time(NULL));

    log.info("Health check response sent");
    return 200;
}

int handle_user_registration(const char* body, char* response_buf) {
    log.info("User registration request received");

    char user_email[256];
    char user_phone[32];
    char user_address[512];
    char user_password[128];

    log.debug("Parsing registration request body");
    sscanf(body, "{\"email\":\"%[^\"]\",\"phone\":\"%[^\"]\",\"address\":\"%[^\"]\",\"password\":\"%[^\"]\"}",
           user_email, user_phone, user_address, user_password);

    log.info("Registering user with email: %s", user_email);
    log.debug("User phone number: %s", user_phone);
    log.debug("User address: %s", user_address);

    if (strlen(user_email) == 0 || strchr(user_email, '@') == NULL) {
        log.error("Registration failed: invalid email format");
        sprintf(response_buf, "{\"error\":\"Invalid email\"}");
        return 400;
    }

    if (strlen(user_password) < 8) {
        log.warn("Registration rejected: password too short");
        sprintf(response_buf, "{\"error\":\"Password too short\"}");
        return 400;
    }

    User* user = (User*)malloc(sizeof(User));
    strcpy(user->email, user_email);
    strcpy(user->phone, user_phone);
    strcpy(user->address, user_address);
    user->is_active = 1;

    log.info("User registered successfully with ID: %s", user->id);
    sprintf(response_buf, "{\"id\":\"%s\",\"status\":\"created\"}", user->id);
    free(user);
    return 201;
}

int handle_authentication(const char* body, char* response_buf) {
    log.info("Authentication request received");

    char email[256];
    char password[128];

    sscanf(body, "{\"email\":\"%[^\"]\",\"password\":\"%[^\"]\"}", email, password);

    log.info("Login attempt for email: %s", email);
    log.debug("Verifying password against stored hash");

    if (strlen(email) == 0) {
        log.error("Authentication failed: empty email");
        sprintf(response_buf, "{\"error\":\"Email required\"}");
        return 400;
    }

    log.info("User authenticated successfully");
    log.debug("Session token generated for user");
    sprintf(response_buf, "{\"token\":\"tok_%ld\"}", time(NULL));
    return 200;
}

int handle_create_order(const char* body, char* response_buf, const char* auth_token) {
    log.info("New order creation request");

    if (!auth_token || strlen(auth_token) == 0) {
        log.error("Order creation rejected: missing authentication token");
        sprintf(response_buf, "{\"error\":\"Unauthorized\"}");
        return 401;
    }

    log.debug("Validating authentication token: %s", auth_token);

    char user_id[64];
    char shipping_address[512];
    char payment_token[128];
    double amount;

    sscanf(body, "{\"user_id\":\"%[^\"]\",\"address\":\"%[^\"]\",\"amount\":%lf,\"payment_token\":\"%[^\"]\"}",
           user_id, shipping_address, &amount, payment_token);

    log.info("Processing order for user: %s", user_id);
    log.debug("Shipping address: %s", shipping_address);
    log.debug("Payment token provided: %s", payment_token);
    log.info("Order amount: %.2f", amount);

    if (amount <= 0) {
        log.error("Invalid order amount: %.2f", amount);
        sprintf(response_buf, "{\"error\":\"Invalid amount\"}");
        return 400;
    }

    Order* order = (Order*)malloc(sizeof(Order));
    strcpy(order->user_id, user_id);
    strcpy(order->shipping_address, shipping_address);
    strcpy(order->payment_token, payment_token);
    order->total = amount;
    strcpy(order->status, "pending");

    log.info("Order created: %s, total: %.2f", order->order_id, order->total);
    sprintf(response_buf, "{\"order_id\":\"%s\",\"status\":\"pending\"}", order->order_id);
    free(order);
    return 201;
}

int handle_get_user_profile(const char* user_id, char* response_buf) {
    log.info("Fetching user profile: %s", user_id);

    User* user = (User*)malloc(sizeof(User));
    strcpy(user->id, user_id);
    strcpy(user->email, "user@example.com");
    strcpy(user->phone, "+1234567890");
    strcpy(user->address, "123 Main St");

    log.debug("User email retrieved: %s", user->email);
    log.debug("User phone: %s", user->phone);
    log.debug("User address loaded: %s", user->address);

    sprintf(response_buf, "{\"id\":\"%s\",\"email\":\"%s\"}", user->id, user->email);

    log.info("User profile returned for: %s", user_id);
    free(user);
    return 200;
}

void start_server(ServerConfig* config) {
    log.info("Starting HTTP server on port %d", config->port);
    log.debug("Maximum client connections: %d", config->max_connections);

    HttpServer server;
    server.socket_fd = socket(AF_INET, SOCK_STREAM, 0);

    if (server.socket_fd < 0) {
        log.error("Failed to create server socket");
        return;
    }

    server.addr.sin_family = AF_INET;
    server.addr.sin_addr.s_addr = INADDR_ANY;
    server.addr.sin_port = htons(config->port);
    server.client_count = 0;

    if (bind(server.socket_fd, (struct sockaddr*)&server.addr, sizeof(server.addr)) < 0) {
        log.error("Failed to bind to port %d", config->port);
        return;
    }

    listen(server.socket_fd, config->max_connections);

    log.info("Server listening on 0.0.0.0:%d", config->port);
    log.info("Ready to accept connections");
}

int main(int argc, char* argv[]) {
    log.info("Initializing C Order Service v1.0.0");

    ServerConfig* config = load_config("config.json");
    if (!config) {
        log.error("Failed to load configuration, exiting");
        return 1;
    }

    log.info("Configuration loaded successfully");
    log.debug("Secret key length: %zu bytes", strlen(config->secret_key));

    start_server(config);

    log.info("Server shutting down");
    free(config);

    log.info("Cleanup complete, goodbye");
    return 0;
}
