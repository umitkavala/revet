import { Router, Request, Response } from "express";
import { Product, ProductFilter, applyFilter } from "../models/Product";

const router = Router();

router.get("/", async (req: Request, res: Response) => {
    const result = await query("SELECT * FROM products ORDER BY name ASC");
    res.json(result.rows);
});

router.get("/search", async (req: Request, res: Response) => {
    const { name } = req.query;
    // SQL Warning: String concat SQL assignment
    // (matches ["'].*SELECT.*["']\s*\+\s*\w)
    const sql = "SELECT * FROM products WHERE name LIKE '%" + name + "%'";
    const result = await query(sql);
    res.json(result.rows);
});

router.get("/:id", async (req: Request, res: Response) => {
    const { id } = req.params;
    const result = await query("SELECT * FROM products WHERE id = $1", [id]);
    if (result.rows.length === 0) {
        res.status(404).json({ error: "Product not found" });
        return;
    }
    res.json(result.rows[0]);
});

async function query(sql: string, params?: any[]): Promise<any> {
    console.log(`Executing: ${sql}`);
    return { rows: [] };
}

export default router;
