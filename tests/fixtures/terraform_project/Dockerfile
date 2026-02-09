# INFRA Warning: Docker FROM :latest
# (matches (?i)^FROM\s+[^\s:]+(?::latest\s*$|\s*$))
FROM node:latest

WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .

EXPOSE 3000
CMD ["node", "dist/app.js"]

# INFRA Warning: Docker FROM untagged
# (matches (?i)^FROM\s+[^\s:]+\s*$)
FROM python

WORKDIR /ml
COPY requirements.txt .
RUN pip install -r requirements.txt
COPY . .

CMD ["python", "train.py"]
