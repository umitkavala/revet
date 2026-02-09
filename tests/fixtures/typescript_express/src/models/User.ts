export interface UserRow {
    id: number;
    username: string;
    email: string;
    created_at: Date;
}

export class User {
    id: number;
    username: string;
    email: string;
    createdAt: Date;

    constructor(row: UserRow) {
        this.id = row.id;
        this.username = row.username;
        this.email = row.email;
        this.createdAt = row.created_at;
    }

    toJSON() {
        return {
            id: this.id,
            username: this.username,
            email: this.email,
            createdAt: this.createdAt.toISOString(),
        };
    }

    isActive(): boolean {
        const thirtyDaysAgo = new Date();
        thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);
        return this.createdAt > thirtyDaysAgo;
    }
}
