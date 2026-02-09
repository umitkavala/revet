export interface Product {
    id: number;
    name: string;
    price: number;
    category: string;
    inStock: boolean;
}

export type ProductFilter = {
    category?: string;
    minPrice?: number;
    maxPrice?: number;
    inStock?: boolean;
};

export function applyFilter(products: Product[], filter: ProductFilter): Product[] {
    return products.filter((p) => {
        if (filter.category && p.category !== filter.category) return false;
        if (filter.minPrice && p.price < filter.minPrice) return false;
        if (filter.maxPrice && p.price > filter.maxPrice) return false;
        if (filter.inStock !== undefined && p.inStock !== filter.inStock) return false;
        return true;
    });
}
