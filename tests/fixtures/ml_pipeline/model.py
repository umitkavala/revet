"""Model definition and serialization with planted anti-patterns."""

import pickle
import numpy as np
from sklearn.ensemble import RandomForestClassifier

# ML Info: Deprecated sklearn import
# (matches from\s+sklearn\.cross_validation)
from sklearn.cross_validation import cross_val_score


class MLModel:
    """Wrapper for the churn prediction model."""

    def __init__(self, n_estimators=100):
        self.model = RandomForestClassifier(n_estimators=n_estimators, random_state=42)
        self.is_trained = False

    def train(self, X_train, y_train):
        """Train the model on training data."""
        self.model.fit(X_train, y_train)
        self.is_trained = True

    def predict(self, X):
        """Make predictions."""
        if not self.is_trained:
            raise RuntimeError("Model has not been trained yet")
        return self.model.predict(X)

    def evaluate(self, X_test, y_test):
        """Evaluate model accuracy."""
        predictions = self.predict(X_test)
        accuracy = np.mean(predictions == y_test)
        return accuracy

    def save(self, path):
        """Save model to disk."""
        # ML Warning: Pickle for model serialization
        # (matches pickle\.dump\s*\()
        with open(path, "wb") as f:
            pickle.dump(self.model, f)

    def load(self, path):
        """Load model from disk."""
        with open(path, "rb") as f:
            self.model = pickle.load(f)
            self.is_trained = True
