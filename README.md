# Pallet Spaces
App that allows one to loan out their pallet spaces to customers

## Goal

Initial goal is a simple web app that allows one to upload pictures of a pallet space with details like how much its being rented for, what time frame its avaliable for and a general description.

Users should eventually get rated on quality(both sellers and buyers)

## Architecture

Typescript front end and Rust backend, moderatly tested.

Docker containers for deployment(maybe nix??? IDK).

Github for initial development, Forgejo if it goes on for a bit, fuck microsoft.

Discord for initial comms, move to matrix if goes on for a while, fuck advertisments.

All documentation should be in the form of markdown files in this repo.

## Desgin

Front page, small blurb about what the site does, followed by examples.

Spaces page, just a grid of spaces for rent, details shown TBD, will need filters eventually but can wait for now.

Space page, an individual space with all its details.

Sign up page, collects an email and whether they want to rent out or rent.

## Making $$$

Transactions can go through the site, we take a small percentage(1-5%).

Later could also offer insurance and logistics services.

Would also be a great place for targeted ads.

## Liability

have a general arbitration clause in TOS, but dont be a dick about it.

## Stripe Customers

- Feature flags: build with `--features stripe` to enable real Stripe HTTP calls. Use `--features stripe,stripe_live` for live tests.
- Env vars:
  - `STRIPE_SECRET_KEY`: required to create/update Stripe Customers.
  - `STRIPE_WEBHOOK_SECRET`: optional; when set, `/webhooks/stripe` logs `customer.updated` events.
  - `ADMIN_EMAIL`: email of the admin account allowed to backfill customers.
- Behavior:
  - On signup and first login, the app ensures a Stripe Customer exists for the user and stores `users.stripe_customer_id`.
  - Email/name updates will be pushed to Stripe when a profile update endpoint is added.
  - Admin backfill endpoint: `POST /admin/stripe/backfill-customers?limit=200&cursor=<last_id>` processes users missing a customer; requires a logged-in session with `email == ADMIN_EMAIL`.
