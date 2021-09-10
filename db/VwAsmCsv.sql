CREATE VIEW IF NOT EXISTS VwAsmLocations AS
    SELECT
        Orgs.sourcedId AS 'location_id'
        , Orgs.name AS 'location_name'
    FROM
        Orgs
;

CREATE VIEW IF NOT EXISTS VwAsmStaff AS
    SELECT
        Users.sourcedId AS 'person_id'
        , NULL AS 'person_number'
        , Users.givenName AS 'first_name'
        , NULL AS 'middle_name'
        , Users.familyName AS 'last_name'
        , Users.email AS 'email_address'
        , NULL AS 'sis_username'
        , json_extract(UO.i, '$[0]') AS 'location_id'
        , json_extract(UO.i, '$[1]') AS 'location_id_2'
        , json_extract(UO.i, '$[2]') AS 'location_id_3'
    FROM
        Users
        JOIN RoleType ON Users.roleTypeId = RoleType.id
        JOIN (
            SELECT
                UserOrgs.userSourcedId
                , json_group_array(UserOrgs.orgSourcedId) AS 'i'
            FROM UserOrgs
            GROUP BY UserOrgs.userSourcedId
        ) AS 'UO' ON Users.sourcedId = UO.userSourcedId
    WHERE
        Users.email IS NOT NULL
        AND RoleType.token = 'teacher'
    ORDER BY 
        Users.sourcedId
;

CREATE VIEW IF NOT EXISTS VwAsmStudents AS
    SELECT
        Users.sourcedId AS 'person_id'
        , NULL AS 'person_number'
        , Users.givenName AS 'first_name'
        , NULL AS 'middle_name'
        , Users.familyName AS 'last_name'
        , NULL AS 'grade_level' -- concat(userGrades)?
        , Users.email AS 'email_address'
        , NULL AS 'sis_username'
        , '4' AS 'password_policy'
        , json_extract(UO.i, '$[0]') AS 'location_id'
        , json_extract(UO.i, '$[1]') AS 'location_id_2'
        , json_extract(UO.i, '$[2]') AS 'location_id_3'
    FROM
        Users
        JOIN RoleType ON Users.roleTypeId = RoleType.id
        JOIN (
            SELECT
                UserOrgs.userSourcedId
                , json_group_array(UserOrgs.orgSourcedId) AS 'i'
            FROM UserOrgs
            GROUP BY UserOrgs.userSourcedId
        ) AS 'UO' ON Users.sourcedId = UO.userSourcedId
    WHERE
        Users.email IS NOT NULL
        AND RoleType.token = 'student'
    ORDER BY 
        Users.sourcedId
;

CREATE VIEW IF NOT EXISTS VwAsmCourses AS
    SELECT 
        Courses.sourcedId AS 'course_id'
        , Courses.courseCode AS 'course_number'
        , Courses.title AS 'course_name'
        , Courses.orgSourcedId AS 'location_id'
    FROM
        Courses
;

CREATE VIEW IF NOT EXISTS VwAsmClasses AS
    SELECT
        Classes.sourcedId AS 'class_id'
        , Classes.classCode AS 'class_number'
        , Classes.courseSourcedId AS 'course_id'
        , json_extract(Ins.i, '$[0]') AS 'instructor_id'
        , json_extract(Ins.i, '$[1]') AS 'instructor_id_2'
        , json_extract(Ins.i, '$[2]') AS 'instructor_id_3'
        , Classes.orgSourcedId AS 'location_id'
    FROM
        Classes
        JOIN (
            SELECT
                Enrollments.classSourcedId
                , json_group_array(Enrollments.userSourcedId) AS 'i'
            FROM Enrollments
            WHERE Enrollments."primary" IS NOT NULL
            GROUP BY Enrollments.classSourcedId
        ) AS 'Ins' ON Classes.sourcedId = Ins.classSourcedId
;

CREATE VIEW IF NOT EXISTS VwAsmRosters AS
    SELECT
        Enrollments.sourcedId AS 'roster_id'
        , Enrollments.classSourcedId AS 'class_id'
        , Enrollments.userSourcedId AS 'student_id'
    FROM
        Enrollments
        JOIN Users ON Enrollments.userSourcedId = Users.sourcedId
        JOIN RoleType ON Users.roleTypeId = RoleType.id
    WHERE
        RoleType.token = 'student'
;
