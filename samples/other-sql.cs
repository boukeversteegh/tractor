class Test
{
    void Example()
    {
        // This should NOT match - different variable name
        var db = new Database();
        db.Sql("""
            SELECT * FROM users
            """);

        // This SHOULD match - correct variable name, missing semicolon
        MigrationBuilder migrationBuilder = null;
        migrationBuilder.Sql("""
            UPDATE users SET active = true
            """);
    }
}
