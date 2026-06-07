# Vercel Deployment Setup Guide

## 🚨 Current Status: Missing Secrets

The deployment workflow failed because the required Vercel secrets are not configured in your GitHub repository settings.

**Error**: `You defined "--token", but it's missing a value`

## 📋 Required Secrets

You need to add these three secrets to your GitHub repository:

1. **VERCEL_TOKEN** - Your Vercel authentication token
2. **VERCEL_ORG_ID** - Your Vercel organization ID
3. **VERCEL_PROJECT_ID** - Your Vercel project ID

---

## 🔧 Step-by-Step Setup Instructions

### Step 1: Get Your Vercel Token

1. Go to [Vercel Account Tokens](https://vercel.com/account/tokens)
2. Click **"Create Token"**
3. Give it a name (e.g., "GitHub Actions - Ghost")
4. Set the scope to your organization
5. Set expiration (recommended: No Expiration for CI/CD)
6. Click **"Create"**
7. **Copy the token immediately** (you won't be able to see it again)

### Step 2: Get Your Vercel Organization and Project IDs

#### Option A: Using Vercel CLI (Recommended)

```bash
# Install Vercel CLI globally
npm install -g vercel@latest

# Navigate to your public directory
cd public

# Login to Vercel (opens browser)
vercel login

# Link your project (follow the prompts)
vercel link

# View the project configuration
cat .vercel/project.json
```

The `.vercel/project.json` file will contain:
```json
{
  "orgId": "team_xxxxxxxxxxxxx",
  "projectId": "prj_xxxxxxxxxxxxx"
}
```

#### Option B: Using Vercel Dashboard

1. Go to [Vercel Dashboard](https://vercel.com/dashboard)
2. Select your project (or create a new one)
3. Go to **Settings** → **General**
4. Find **Project ID** (copy it)
5. Find **Team ID** or **Organization ID** (copy it)

### Step 3: Add Secrets to GitHub Repository

1. Go to your GitHub repository: `https://github.com/mohabbis/ghost`
2. Click **Settings** (top menu)
3. In the left sidebar, click **Secrets and variables** → **Actions**
4. Click **"New repository secret"**
5. Add each secret:

   **Secret 1:**
   - Name: `VERCEL_TOKEN`
   - Value: `[paste your Vercel token]`
   - Click **"Add secret"**

   **Secret 2:**
   - Name: `VERCEL_ORG_ID`
   - Value: `[paste your org ID, e.g., team_xxxxxxxxxxxxx]`
   - Click **"Add secret"**

   **Secret 3:**
   - Name: `VERCEL_PROJECT_ID`
   - Value: `[paste your project ID, e.g., prj_xxxxxxxxxxxxx]`
   - Click **"Add secret"**

---

## 🚀 Deploy Your Website

### Option 1: Trigger Deployment via Push

Once secrets are added, push any change to the `public/` directory:

```bash
# Make a small change to trigger deployment
cd public
echo "<!-- Updated $(date) -->" >> index.html

# Commit and push
git add .
git commit -m "Trigger Vercel deployment"
git push origin master
```

### Option 2: Manual Workflow Trigger

```bash
# Trigger the workflow manually
gh workflow run deploy-website.yml
```

### Option 3: Re-run Failed Workflow

```bash
# Re-run the failed workflow (run ID: 27101555418)
gh run rerun 27101555418
```

---

## 🔍 Verify Deployment

After the workflow completes successfully:

1. **Check GitHub Actions**: 
   - Go to `https://github.com/mohabbis/ghost/actions`
   - Look for the "Deploy Website" workflow
   - Verify it shows a green checkmark ✅

2. **Visit Your Website**:
   - Open `https://ghost.muharafiq.com`
   - Verify the content is correct

3. **Test Download Links**:
   - macOS: `https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg`
   - Windows: `https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe`

4. **Validate SEO**:
   - Test with [Google Rich Results](https://search.google.com/test/rich-results?url=https://ghost.muharafiq.com)
   - Test with [Schema Validator](https://validator.schema.org/)

---

## 📦 What Gets Deployed

The workflow deploys everything in the `public/` directory:

```
public/
├── index.html          # Main landing page
├── styles.css          # Styling
├── main.js            # JavaScript
├── favicon.svg        # Favicon
├── apple-touch-icon.png
└── assets/            # Images and resources
```

---

## 🔐 Security Best Practices

1. **Token Scope**: Create a token with minimal required permissions
2. **Token Expiration**: Set an expiration date if possible
3. **Rotate Tokens**: Periodically rotate your Vercel token
4. **Monitor Usage**: Check Vercel dashboard for unexpected deployments
5. **Audit Logs**: Review GitHub Actions logs regularly

---

## 🐛 Troubleshooting

### Issue: "Invalid token" error

**Solution**: 
- Verify the token is copied correctly (no extra spaces)
- Check token hasn't expired
- Ensure token has correct permissions

### Issue: "Project not found" error

**Solution**:
- Verify `VERCEL_PROJECT_ID` is correct
- Ensure project exists in Vercel dashboard
- Check you're using the right organization

### Issue: "Organization not found" error

**Solution**:
- Verify `VERCEL_ORG_ID` is correct
- Ensure you have access to the organization
- Check if it's a personal account (use your user ID instead)

### Issue: Deployment succeeds but site doesn't update

**Solution**:
- Clear browser cache
- Check Vercel dashboard for deployment status
- Verify correct domain is configured in Vercel

---

## 📚 Additional Resources

- [Vercel CLI Documentation](https://vercel.com/docs/cli)
- [GitHub Actions Secrets](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
- [Vercel Deployment Documentation](https://vercel.com/docs/deployments/overview)

---

## ✅ Checklist

Before deploying, ensure:

- [ ] Vercel account is set up
- [ ] Project is created in Vercel (or will be created automatically)
- [ ] Domain `ghost.muharafiq.com` is configured in Vercel
- [ ] `VERCEL_TOKEN` secret is added to GitHub
- [ ] `VERCEL_ORG_ID` secret is added to GitHub
- [ ] `VERCEL_PROJECT_ID` secret is added to GitHub
- [ ] `public/` directory contains all necessary files
- [ ] Download links in `index.html` point to correct GitHub releases

---

## 🎯 Quick Commands Reference

```bash
# Install Vercel CLI
npm install -g vercel@latest

# Login to Vercel
vercel login

# Link project
cd public && vercel link

# View project config
cat .vercel/project.json

# Manual deployment (for testing)
cd public && vercel --prod

# Trigger GitHub Actions workflow
gh workflow run deploy-website.yml

# Watch workflow execution
gh run watch

# Re-run failed workflow
gh run rerun 27101555418
```

---

**Need Help?** Check the [Vercel Support](https://vercel.com/support) or [GitHub Actions Documentation](https://docs.github.com/en/actions).