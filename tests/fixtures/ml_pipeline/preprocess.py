"""Data preprocessing module with intentionally planted ML anti-patterns."""

import pandas as pd
import numpy as np
from sklearn.preprocessing import StandardScaler
from sklearn.model_selection import train_test_split


def load_data():
    """Load dataset from disk."""
    # ML Warning: Hardcoded absolute data path
    # (matches \.read_csv\s*\(\s*["']/)
    df = pd.read_csv("/data/raw/customer_churn.csv")
    return df


def feature_engineering(df):
    """Create features from raw data."""
    df["tenure_months"] = df["tenure_days"] / 30
    df["spend_ratio"] = df["monthly_spend"] / df["total_spend"].clip(lower=1)
    return df


def split_data(df):
    """Split data into train and test sets."""
    X = df.drop("churned", axis=1)
    y = df["churned"]

    # ML Warning: train_test_split without random_state
    # (matches train_test_split\s*\( and rejects lines containing random_state)
    X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2)

    return X_train, X_test, y_train, y_test


def scale_features(X):
    """Scale features using StandardScaler."""
    scaler = StandardScaler()
    # ML Warning: fit_transform on full dataset (before splitting)
    # (matches \.fit_transform\s*\(\s*X\s*[\),])
    X_scaled = scaler.fit_transform(X)
    return X_scaled, scaler
