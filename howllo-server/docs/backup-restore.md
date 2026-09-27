# Backup and restore (preview)

Howllo state spans PostgreSQL **and** object storage. A database dump by itself
does not restore uploaded screenshots, board images, or logos. Keep the
authentication, OIDC, storage and encryption secrets used by the installation
in a separate encrypted backup. Do not commit them to Git.

## Back up

1. Pause writes or use a storage snapshot strategy that gives the database and
   objects a consistent point in time.
2. Back up PostgreSQL using a custom-format dump:

   ```bash
   pg_dump --format=custom --file=howllo.dump "$HOWLLO_DATABASE_URL"
   ```

3. Back up the entire configured MinIO bucket (or the local upload directory)
   with your object-store backup tool. Preserve object keys and metadata. The
   database stores references to these keys.
4. Back up the installation's environment secrets and deployment config in an
   encrypted location. Record the Howllo Git commit or release tag that created
   the backup.

## Restore to an isolated installation

1. Start the **same Howllo version** with an empty database and empty object
   bucket, but keep the API stopped until the data is restored.
2. Restore the database into the empty database:

   ```bash
   pg_restore --no-owner --dbname "$HOWLLO_DATABASE_URL" howllo.dump
   ```

3. Restore the object-store backup under the same bucket and keys. Configure
   the matching storage endpoint, bucket and secrets. Restore the other saved
   authentication and encryption secrets.
4. Start the API and check `/api/ready`. Verify staff sign-in, a customer
   board, and an uploaded image. Check that the restored object count and total
   bytes match the backup.

Do not run newer migrations against the backup until a restore with the matching
version has passed. SQLx records applied migrations and verifies their checksums;
the migration files are **not** generally replayable or reversible. Take a new
backup before every version upgrade. A scheduled restore test and a complete
reference deployment are still release gates in [the roadmap](../../docs/roadmap.md).
