-- Simple T-SQL example
SELECT u.Name, u.Age
FROM Users u
WHERE u.Age > 18
ORDER BY u.Name;

INSERT INTO AuditLog (Action, Timestamp)
VALUES ('UserQuery', GETDATE());

DELETE FROM OldRecords WHERE CreatedAt < '2020-01-01';

-- Bracket identifiers, schema-qualified names, variables
SELECT [dbo].[Users].[Name] AS UserName, @StartDate
FROM [dbo].[Users]
WHERE [Age] BETWEEN 18 AND 65;

-- UPDATE with variables and schema-qualified table
UPDATE dbo.Config SET Value = @NewValue WHERE [Key] = @KeyName;

-- JOINs
SELECT o.ID, u.Name, a.City
FROM Orders o
JOIN Users u ON o.UserID = u.ID
LEFT JOIN Addresses a ON u.ID = a.UserID
WHERE o.Total > 100;

-- Subquery with IN
SELECT Name FROM Users
WHERE ID IN (SELECT UserID FROM Orders WHERE Total > 50);

-- EXISTS
SELECT u.Name FROM Users u
WHERE EXISTS (SELECT 1 FROM Orders o WHERE o.UserID = u.ID);

-- GROUP BY, HAVING, aggregate functions
SELECT Department, COUNT(*) AS HeadCount, AVG(Salary) AS AvgSalary
FROM Employees
GROUP BY Department
HAVING COUNT(*) > 5;

-- CASE WHEN
SELECT Name,
    CASE WHEN Age >= 18 THEN 'Adult' ELSE 'Minor' END AS Category
FROM Users;

-- CTE (Common Table Expression)
WITH ActiveUsers AS (
    SELECT ID, Name FROM Users WHERE Active = 1
)
SELECT Name FROM ActiveUsers;

-- UNION ALL
SELECT Name, 'User' AS Source FROM Users
UNION ALL
SELECT Name, 'Admin' AS Source FROM Admins;

-- Window function with PARTITION BY
SELECT Name,
    ROW_NUMBER() OVER (PARTITION BY Department ORDER BY Salary DESC) AS Rank
FROM Employees;

-- COALESCE, CAST, LIKE, IS NULL, IN list
SELECT
    COALESCE(Nickname, Name) AS DisplayName,
    CAST(Age AS VARCHAR(10)) AS AgeText
FROM Users
WHERE Email LIKE '%@gmail.com'
    AND DeletedAt IS NULL
    AND Role IN ('admin', 'moderator');

-- DISTINCT with ORDER BY DESC
SELECT DISTINCT Department FROM Employees ORDER BY Department DESC;

-- CREATE TABLE
CREATE TABLE Audit (
    ID INT PRIMARY KEY,
    Action NVARCHAR(100) NOT NULL,
    CreatedAt DATETIME DEFAULT GETDATE()
);

-- MERGE statement
MERGE INTO Target t
USING Source s ON t.ID = s.ID
WHEN MATCHED THEN UPDATE SET t.Name = s.Name
WHEN NOT MATCHED THEN INSERT (ID, Name) VALUES (s.ID, s.Name);

-- SELECT INTO temp table
SELECT Name, Age INTO #TempUsers FROM Users WHERE Active = 1;

-- Transaction
BEGIN TRANSACTION;
UPDATE Accounts SET Balance = Balance - 100 WHERE ID = 1;
UPDATE Accounts SET Balance = Balance + 100 WHERE ID = 2;
COMMIT;

-- SET variable
SET @Threshold = 42;

-- Scalar function
CREATE FUNCTION dbo.GetAge(@BirthDate DATE)
RETURNS INT
AS
BEGIN
    RETURN DATEDIFF(YEAR, @BirthDate, GETDATE())
END;

GO

-- EXEC stored procedure
EXEC sp_helpdb;

-- Iter 21: new shapes — DDL variants.

-- CREATE VIEW, DROP TABLE
CREATE VIEW ActiveUsers AS SELECT ID, Name FROM Users WHERE Active = 1;
DROP TABLE TempData;

-- ALTER TABLE: add constraint, drop constraint
ALTER TABLE Orders ADD CONSTRAINT fk_customer FOREIGN KEY (CustomerID) REFERENCES Customers(ID);
ALTER TABLE Orders DROP CONSTRAINT fk_customer;
