public class UserRecord
{
    [MaxLength(1)]
    public bool IsActive { get; set; }  // ERROR - MaxLength on bool

    [MaxLength(100)]
    public string Name { get; set; }  // OK - MaxLength on string
}
