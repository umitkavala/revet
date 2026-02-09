import { Router, Request, Response } from "express";
import { db } from "../services/database";
import { User } from "../models/User";

const router = Router();

router.get("/", async (req: Request, res: Response) => {
    const users = await db.getAllUsers();
    res.json(users.map((u) => u.toJSON()));
});

router.get("/search", async (req: Request, res: Response) => {
    const { username } = req.query;
    // SQL Error: Template literal SQL in query call
    // (matches \.query\s*\(\s*`[^`]*SELECT[^`]*\$\{[^`]*`)
    const result = await db.query(`SELECT * FROM users WHERE username = '${username}'`);
    res.json(result.rows);
});

router.get("/:id", async (req: Request, res: Response) => {
    const { id } = req.params;
    const result = await db.query("SELECT * FROM users WHERE id = $1", [id]);
    if (result.rows.length === 0) {
        res.status(404).json({ error: "User not found" });
        return;
    }
    const user = new User(result.rows[0]);
    res.json(user.toJSON());
});

export default router;
