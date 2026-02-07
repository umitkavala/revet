"""
Sample Flask app with intentional security issues for testing.

This file contains INTENTIONAL vulnerabilities for testing purposes.
DO NOT use this code in production!
"""

from flask import Flask, request
import sqlite3

app = Flask(__name__)


@app.route("/user/<user_id>")
def get_user(user_id):
    """Get user by ID - VULNERABLE to SQL injection."""
    conn = sqlite3.connect("database.db")
    cursor = conn.cursor()

    # INTENTIONAL SQL INJECTION VULNERABILITY
    query = f"SELECT * FROM users WHERE id = {user_id}"
    cursor.execute(query)

    result = cursor.fetchone()
    conn.close()
    return {"user": result}


@app.route("/search")
def search():
    """Search users - VULNERABLE to SQL injection."""
    term = request.args.get("q", "")

    conn = sqlite3.connect("database.db")
    cursor = conn.cursor()

    # INTENTIONAL SQL INJECTION VULNERABILITY
    cursor.execute("SELECT * FROM users WHERE name LIKE '%" + term + "%'")

    results = cursor.fetchall()
    conn.close()
    return {"results": results}


# INTENTIONAL: Hardcoded secret (test fixture only - not a real key)
# This triggers Revet's secret detection for testing purposes
API_KEY = "hardcoded_api_key_example_not_real_12345"


if __name__ == "__main__":
    app.run(debug=True)
