"""Training script with intentionally planted data leakage issues."""

from sklearn.preprocessing import StandardScaler, LabelEncoder
from sklearn.model_selection import train_test_split
from preprocess import load_data, feature_engineering
from model import MLModel


def main():
    # Load and prepare data
    df = load_data()
    df = feature_engineering(df)

    X = df.drop("churned", axis=1)
    y = df["churned"]

    # ML Info: train_test_split without stratify
    # (matches train_test_split\s*\(, requires random_state, rejects stratify)
    X_train, X_test, y_train, y_test = train_test_split(
        X, y, test_size=0.2, random_state=42
    )

    # ML Error: Fit on test data (data leakage)
    # (matches \.fit\s*\(.*X_test)
    scaler = StandardScaler()
    scaler.fit(X_test)
    X_train_scaled = scaler.transform(X_train)
    X_test_scaled = scaler.transform(X_test)

    # ML Error: Fit on test labels (data leakage)
    # (matches \.fit\s*\(.*y_test)
    encoder = LabelEncoder()
    encoder.fit(y_test)

    # Train model
    model = MLModel(n_estimators=200)
    model.train(X_train_scaled, y_train)

    # Evaluate
    accuracy = model.evaluate(X_test_scaled, y_test)
    print(f"Test accuracy: {accuracy:.4f}")

    # Save model
    model.save("model.pkl")


if __name__ == "__main__":
    main()
