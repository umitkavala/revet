import express from "express";
import userRoutes from "./routes/users";
import productRoutes from "./routes/products";
import { config } from "./config";

const app = express();

app.use(express.json());
app.use("/api/users", userRoutes);
app.use("/api/products", productRoutes);

app.get("/health", (req, res) => {
    res.json({ status: "ok" });
});

app.listen(config.port, () => {
    console.log(`Server running on http://${config.host}:${config.port}`);
});

export default app;
