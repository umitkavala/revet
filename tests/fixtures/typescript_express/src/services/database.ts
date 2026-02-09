import { config } from "../config";
import { User, UserRow } from "../models/User";

// SEC Warning: Hardcoded password (matches password\s*[:=]\s*['"][^'"]{8,}['"])
const DB_CONFIG = {
    host: "localhost",
    port: 5432,
    database: "express_app",
    user: "admin",
    password: "SuperSecretP@ssw0rd!",
};

export class DatabaseService {
    private connectionString: string;

    constructor() {
        this.connectionString = `postgres://${DB_CONFIG.user}:${DB_CONFIG.password}@${DB_CONFIG.host}:${DB_CONFIG.port}/${DB_CONFIG.database}`;
    }

    // SQL Error: Template literal SQL in DB call
    // (matches \.query\s*\(\s*`[^`]*SELECT[^`]*\$\{[^`]*`)
    async findUserByEmail(email: string): Promise<User | null> {
        const result = await this.query(`SELECT * FROM users WHERE email = '${email}'`);
        if (result.rows.length === 0) return null;
        return new User(result.rows[0] as UserRow);
    }

    async getAllUsers(): Promise<User[]> {
        const result = await this.query("SELECT * FROM users ORDER BY created_at DESC");
        return result.rows.map((row: UserRow) => new User(row));
    }

    private async query(sql: string): Promise<any> {
        // Simulated database query
        console.log(`Executing: ${sql}`);
        return { rows: [] };
    }
}

export const db = new DatabaseService();
