using Microsoft.EntityFrameworkCore.Migrations;

namespace Test
{
    public partial class BadMigration : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            // MISSING semicolon!
            migrationBuilder.Sql("""
                UPDATE "Users"
                SET "Active" = true
                WHERE "CreatedAt" < NOW()
                """);
        }
    }
}
