public class UserRecord
{
    [MaxLength(100)]
    public string Name { get; set; }  // MATCH - missing AutoTruncate

    [MaxLength(50)]
    [AutoTruncate]
    public string Email { get; set; }  // OK - has AutoTruncate

    public string Bio { get; set; }  // OK - no MaxLength
}
