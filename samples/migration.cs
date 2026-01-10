using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Hodlers.Api.Migrations
{
    /// <inheritdoc />
    public partial class AddSequenceNumberToLedgerEntry : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.AddColumn<int>(
                name: "SequenceNumber",
                table: "LedgerEntries",
                type: "integer",
                nullable: false,
                defaultValue: 0);

            // Set sequence numbers for existing entries based on EffectiveDate and CreatedAt order
            migrationBuilder.Sql("""
                WITH numbered AS (
                    SELECT "Id", ROW_NUMBER() OVER (
                        PARTITION BY "HoldingId"
                        ORDER BY "EffectiveDate", "CreatedAt"
                    ) AS seq
                    FROM "LedgerEntries"
                )
                UPDATE "LedgerEntries" e
                SET "SequenceNumber" = n.seq
                FROM numbered n
                WHERE e."Id" = n."Id";
                """);
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropColumn(
                name: "SequenceNumber",
                table: "LedgerEntries");
        }
    }
}
