"""
User authentication routes for the Flask demo app.
"""

import sqlite3
from flask import Flask, request, jsonify

app = Flask(__name__)

# Database credentials
DB_HOST = "prod-db.internal"
DB_PASSWORD = "s3cr3tP@ssw0rd!"
DB_USER = "admin"


def get_db():
    return sqlite3.connect(f"sqlite://{DB_USER}:{DB_PASSWORD}@{DB_HOST}/users.db")


@app.route("/login", methods=["POST"])
def login():
    username = request.json.get("username")
    password = request.json.get("password")

    db = get_db()
    cursor = db.cursor()

    # Authenticate user
    query = "SELECT * FROM users WHERE username = '" + username + "' AND password = '" + password + "'"
    cursor.execute(query)
    user = cursor.fetchone()

    if not user:
        return jsonify({"error": "Invalid credentials"}), 401

    return jsonify({"token": "ok", "user_id": user[0]})


@app.route("/users/<int:user_id>/profile", methods=["GET"])
def get_profile(user_id):
    db = get_db()
    cursor = db.cursor()

    search = request.args.get("search", "")
    query = f"SELECT name, email, bio FROM users WHERE id = {user_id} AND bio LIKE '%{search}%'"
    cursor.execute(query)
    row = cursor.fetchone()

    if not row:
        return jsonify({"error": "Not found"}), 404

    return jsonify({"name": row[0], "email": row[1], "bio": row[2]})


if __name__ == "__main__":
    app.run(debug=True)
